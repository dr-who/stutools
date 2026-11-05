//! The NVMe over Fabrics cases of `--compliance` (Base 2.1 Fabrics: Connect, properties, associations):
//! - nvme-conn: Fabrics Connect returns a controller ID, and a second association alongside the first
//!   gets another one;
//! - nvme-prop: CAP reports queue entries and the NVM command set, VS is 1.0 or later, and enabling
//!   the controller sets CSTS.RDY;
//! - nvme-sess: three associations in turn connect and enable.
//!
//! `connect` and `enable` hand the connected, enabled controller to the later sections.

use super::controller::{self, Association, Controller};
use crate::compliance::{case, Case};

/// The VS property: the specification version the controller implements.
const VS: u32 = 0x08;

/// What the property cases read: CAP, and VS when it could be read.
pub struct Properties {
    pub cap: u64,
    pub vs: Option<u64>,
}

/// Connects the admin queue, and a second association alongside it. None when Connect fails.
pub fn connect(assoc: &Association, name: &str, cases: &mut Vec<Case>) -> Option<Controller> {
    let c = match Controller::connect(assoc) {
        Ok(c) => c,
        Err(e) => {
            cases.push(case(
                "nvme-conn",
                format!("{name} connect"),
                "a controller ID".into(),
                e,
                false,
            ));
            return None;
        }
    };
    cases.push(case(
        "nvme-conn",
        format!("{name} connect"),
        "a controller ID".into(),
        format!("{:#06x}", c.cntlid()),
        c.cntlid() != 0xffff,
    ));
    let second = Controller::connect(assoc);
    cases.push(case(
        "nvme-conn",
        format!("{name} a second association"),
        "another controller ID".into(),
        match &second {
            Ok(s) => format!("{:#06x}", s.cntlid()),
            Err(e) => e.clone(),
        },
        second.as_ref().is_ok_and(|s| s.cntlid() != c.cntlid()),
    ));
    Some(c)
}

/// Reads CAP and VS, then enables the controller. None when it does not become ready.
pub fn enable(c: &mut Controller, name: &str, cases: &mut Vec<Case>) -> Option<Properties> {
    let cap = c.property_get(controller::CAP, true);
    cases.push(case(
        "nvme-prop",
        format!("{name} CAP"),
        "MQES > 0, the NVM command set".into(),
        match &cap {
            Ok(v) => format!("MQES {}, CSS {:#x}", v & 0xffff, (v >> 37) & 0xff),
            Err(e) => e.clone(),
        },
        cap.as_ref()
            .is_ok_and(|v| v & 0xffff > 0 && (v >> 37) & 1 == 1),
    ));
    let vs = c.property_get(VS, false);
    cases.push(case(
        "nvme-prop",
        format!("{name} VS"),
        "1.0 or later".into(),
        match &vs {
            Ok(v) => format!("{}.{}", v >> 16, (v >> 8) & 0xff),
            Err(e) => e.clone(),
        },
        vs.as_ref().is_ok_and(|v| v >> 16 >= 1),
    ));
    let enabled = c.enable();
    cases.push(case(
        "nvme-prop",
        format!("{name} enable"),
        "CSTS.RDY".into(),
        match &enabled {
            Ok(()) => "ready".into(),
            Err(e) => e.clone(),
        },
        enabled.is_ok(),
    ));
    enabled.ok().map(|()| Properties {
        cap: cap.unwrap_or(0),
        vs: vs.ok(),
    })
}

/// Three associations in turn, each connected and enabled, then closed.
pub fn sessions(assoc: &Association, name: &str, cases: &mut Vec<Case>) {
    for k in 1..=3 {
        let result = Controller::connect(assoc).and_then(|mut c| c.enable().map(|()| c.cntlid()));
        cases.push(case(
            "nvme-sess",
            format!("{name} association {k}"),
            "connect and enable".into(),
            match &result {
                Ok(id) => format!("controller {id:#06x}"),
                Err(e) => e.clone(),
            },
            result.is_ok(),
        ));
    }
}
