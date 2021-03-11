use super::errors::*;
use crate::{Conn, Error};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

type PortMap = Mutex<HashMap<u16, Vec<Arc<Mutex<Box<dyn Conn + Send + Sync>>>>>>;

pub(crate) struct UDPConnMap {
    port_map: PortMap,
}

impl UDPConnMap {
    pub(crate) fn new() -> Self {
        UDPConnMap {
            port_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn insert(
        &self,
        conn: Arc<Mutex<Box<dyn Conn + Send + Sync>>>,
    ) -> Result<(), Error> {
        let addr = {
            let c = conn.lock().await;
            c.local_addr()?
        };

        let mut port_map = self.port_map.lock().await;
        if let Some(conns) = port_map.get(&addr.port()) {
            for cs in conns {
                let c = cs.lock().await;
                let laddr = c.local_addr()?;
                if laddr.ip() == addr.ip() {
                    return Err(ERR_ADDRESS_ALREADY_IN_USE.to_owned());
                }
            }
        }

        if let Some(conns) = port_map.get_mut(&addr.port()) {
            conns.push(conn);
        } else {
            port_map.insert(addr.port(), vec![conn]);
        }
        Ok(())
    }

    pub(crate) async fn find(
        &self,
        addr: &SocketAddr,
    ) -> Option<Arc<Mutex<Box<dyn Conn + Send + Sync>>>> {
        let port_map = self.port_map.lock().await;
        if let Some(conns) = port_map.get(&addr.port()) {
            for cs in conns {
                let laddr = {
                    let c = cs.lock().await;
                    match c.local_addr() {
                        Ok(laddr) => laddr,
                        Err(_) => return None,
                    }
                };
                if laddr.ip() == addr.ip() {
                    return Some(Arc::clone(&cs));
                }
            }
        }

        None
    }

    pub(crate) async fn delete(&self, addr: &SocketAddr) -> Result<(), Error> {
        let mut port_map = self.port_map.lock().await;
        let mut new_conns = vec![];
        if let Some(conns) = port_map.get(&addr.port()) {
            for cs in conns {
                let c = cs.lock().await;
                let laddr = c.local_addr()?;
                if laddr.ip() == addr.ip() {
                    continue;
                }
                new_conns.push(Arc::clone(cs));
            }
        }

        if new_conns.is_empty() {
            port_map.remove(&addr.port());
        } else {
            port_map.insert(addr.port(), new_conns);
        }

        Ok(())
    }

    pub(crate) async fn len(&self) -> usize {
        let port_map = self.port_map.lock().await;
        let mut n = 0;
        for conns in port_map.values() {
            n += conns.len();
        }
        n
    }
}
