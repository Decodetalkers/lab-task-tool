use zbus::proxy;

use common::*;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::sync::OnceLock;

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

#[derive(Debug, Clone, Default)]
pub struct UnitInterfaceInfoVec(Vec<TaskInfo>);

impl UnitInterfaceInfoVec {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TaskInfo> {
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
            let Some(task) = get_task_information(&id) else {
                continue;
            };
            unitvec.push(task);
        }
        Ok(Self(unitvec))
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
}
#[tokio::main]
async fn main() {
    let infos = UnitInterfaceInfoVec::refresh().await.unwrap();
    for info in infos.iter() {
        println!("{info:?}");
    }
}
