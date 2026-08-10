use std::{io, thread};

use crate::{
    agents::{AgentInspection, inspect_current_agents},
    doctor::{DoctorProbe, probe_doctor},
    features::{FeaturesProbe, probe_features},
    marketplaces::{MarketplaceProbe, probe_marketplaces},
    mcp::{McpProbe, probe_mcp},
    plugins::{PluginProbe, probe_plugins},
    skills::{ActiveSkillSummary, inspect_active_skills},
};

#[derive(Debug)]
pub(crate) struct InspectionProbes {
    pub(crate) doctor: io::Result<DoctorProbe>,
    pub(crate) features: io::Result<FeaturesProbe>,
    pub(crate) plugins: io::Result<PluginProbe>,
    pub(crate) marketplaces: io::Result<MarketplaceProbe>,
    pub(crate) mcp: io::Result<McpProbe>,
    pub(crate) agents: io::Result<AgentInspection>,
    pub(crate) skills: io::Result<ActiveSkillSummary>,
}

pub(crate) fn probe_inspection() -> InspectionProbes {
    let doctor_worker = thread::spawn(probe_doctor);
    let features_worker = thread::spawn(probe_features);
    let plugins_worker = thread::spawn(probe_plugins);
    let marketplaces_worker = thread::spawn(probe_marketplaces);
    let mcp_worker = thread::spawn(probe_mcp);
    let agents_worker = thread::spawn(inspect_current_agents);
    let skills_worker = thread::spawn(inspect_active_skills);

    let doctor = join_worker(doctor_worker, "doctor");
    let features = join_worker(features_worker, "features");
    let plugins = join_worker(plugins_worker, "plugins");
    let marketplaces = join_worker(marketplaces_worker, "marketplaces");
    let mcp = join_worker(mcp_worker, "mcp");
    let agents = join_worker(agents_worker, "agents");
    let skills = join_worker(skills_worker, "skills");

    InspectionProbes {
        doctor,
        features,
        plugins,
        marketplaces,
        mcp,
        agents,
        skills,
    }
}

fn join_worker<T>(worker: thread::JoinHandle<io::Result<T>>, name: &str) -> io::Result<T> {
    worker
        .join()
        .unwrap_or_else(|_| Err(io::Error::other(format!("{name} worker panicked"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_worker_panic_to_io_error() {
        let worker = thread::spawn(|| -> io::Result<()> {
            panic!("fixture panic");
        });

        let error = join_worker(worker, "fixture").expect_err("worker should fail");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "fixture worker panicked");
    }
}
