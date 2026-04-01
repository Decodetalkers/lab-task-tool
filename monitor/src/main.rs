mod cli;

use clap::Parser;
use cli_table::format::Justify;
use cli_table::{Cell, CellStruct, Style, Table};
use dialoguer::FuzzySelect;
use dialoguer::theme::ColorfulTheme;
use zbus::proxy;

use common::*;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::ops::Index;
use std::sync::OnceLock;
use zbus::zvariant::OwnedObjectPath;

static SESSION: OnceLock<zbus::Connection> = OnceLock::new();

#[derive(Debug, thiserror::Error, Clone)]
pub enum UnitGetError {
    #[error("Error during zbus tokio thread")]
    ZbusThreadError(#[from] zbus::Error),
    #[error("Xml is broken")]
    XmlError,
}

async fn get_connection() -> zbus::Result<zbus::Connection> {
    if let Some(cnx) = SESSION.get() {
        Ok(cnx.clone())
    } else {
        let cnx = zbus::Connection::session().await?;
        SESSION.set(cnx.clone()).expect("Can't reset a OnceCell");
        Ok(cnx)
    }
}

fn names_from_xml(xml: String) -> Result<Vec<String>, quick_xml::Error> {
    let mut interfaces = Vec::new();
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    reader.config_mut().expand_empty_elements = true;
    let mut buf = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buf)?;
        match event {
            Event::Start(element) => {
                if let b"node" = element.name().as_ref() {
                    for att in element.attributes().flatten() {
                        if att.key.as_ref() == b"name"
                            && let Ok(name) = att.decode_and_unescape_value(reader.decoder())
                        {
                            interfaces.push(name.to_string());
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(interfaces)
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct UnitInfo {
    originunit: String,
    can_freeze: bool,
    collect_mode: String,
    id: String,
}

#[derive(Debug)]
pub struct SystemTask<'a> {
    info: TaskInfo,
    active_state: String,
    dbus: Systemd1UnitProxy<'a>,
}
impl<'a> SystemTask<'a> {
    pub async fn restart(&self) {
        self.dbus.restart("replace").await.unwrap();
    }
    pub async fn stop(&self) {
        self.dbus.stop("replace").await.unwrap();
    }
    pub async fn reset_failed(&self) {
        self.dbus.reset_failed().await.unwrap();
    }
    pub fn is_failed(&self) -> bool {
        self.active_state == "failed"
    }
}

#[derive(Debug, Default)]
pub struct UnitInterfaceInfoVec<'a>(Vec<SystemTask<'a>>);

impl<'a> UnitInterfaceInfoVec<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn ids(&'_ self) -> impl Iterator<Item = &'_ str> {
        self.0.iter().map(|t| t.info.id.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&'_ self) -> impl Iterator<Item = &'_ SystemTask<'_>> {
        self.0.iter()
    }

    pub async fn refresh() -> Result<Self, UnitGetError> {
        let conn = get_connection().await?;
        let systembus = SystemdDbusProxy::new(&conn).await?;
        let xml = systembus.introspect().await?;
        let mut unitvec = Vec::new();
        let names = names_from_xml(xml).map_err(|_| UnitGetError::XmlError)?;
        for unit in names {
            let unitbus = Systemd1UnitProxy::builder(&conn)
                .path(format!("/org/freedesktop/systemd1/unit/{unit}"))?
                .build()
                .await?;
            let id = unitbus.id().await?;
            let active_state = unitbus.active_state().await?;
            let Some(task) = get_task_information(&id) else {
                continue;
            };
            unitvec.push(SystemTask {
                info: task,
                active_state,
                dbus: unitbus,
            });
        }
        Ok(Self(unitvec))
    }
}

impl<'a> Index<usize> for UnitInterfaceInfoVec<'a> {
    type Output = SystemTask<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[proxy(
    default_service = "org.freedesktop.systemd1",
    interface = "org.freedesktop.DBus.Introspectable",
    default_path = "/org/freedesktop/systemd1/unit"
)]
trait SystemdDbus {
    fn introspect(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait Systemd1Unit {
    #[zbus(property)]
    fn can_freeze(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn collect_mode(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn active_state(&self) -> zbus::Result<String>;

    fn restart(&self, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn stop(&self, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn reset_failed(&self) -> zbus::Result<()>;
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg = cli::Cli::parse();
    use cli::Commands;
    let infos = UnitInterfaceInfoVec::refresh().await?;
    match arg.command {
        Commands::Status => {
            let display = infos
                .iter()
                .map(|info| {
                    vec![
                        info.info.task.clone().cell(),
                        info.info.time.clone().cell(),
                        info.active_state.clone().cell(),
                    ]
                })
                .collect::<Vec<Vec<CellStruct>>>()
                .table()
                .title(vec![
                    "Task".cell().justify(Justify::Left).bold(true),
                    "Time".cell().justify(Justify::Center).bold(true),
                    "Status".cell().justify(Justify::Center).bold(true),
                ])
                .bold(true)
                .display()?;

            println!("{display}");
        }
        Commands::Restart => {
            let choice = choose_command(infos.ids());
            if choice == -1 {
                eprintln!("You have not choose a task");
                return Ok(());
            }
            let info = &infos[choice as usize];
            info.restart().await;
        }
        Commands::ResetFailed => {
            let failed_units: Vec<&SystemTask<'_>> =
                infos.iter().filter(|task| task.is_failed()).collect();
            let choice = choose_command(failed_units.iter().map(|task| task.info.id.as_str()));
            if choice == -1 {
                eprintln!("You have not choose a task");
                return Ok(());
            }
            failed_units[choice as usize].reset_failed().await;
        }
        Commands::Stop => {
            let choice = choose_command(infos.ids());
            if choice == -1 {
                eprintln!("You have not choose a task");
                return Ok(());
            }
            infos[choice as usize].stop().await;
        }
    }

    Ok(())
}

fn choose_command<'a, T>(titles: T) -> i32
where
    T: Iterator<Item = &'a str>,
{
    let Ok(index) = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Now to choose a command")
        .default(0)
        .items(titles)
        .interact()
    else {
        return -1;
    };
    index as i32
}
