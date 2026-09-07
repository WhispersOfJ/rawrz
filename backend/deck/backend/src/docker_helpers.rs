pub fn runtime_docker(runtime: &crate::probes::Runtime) -> Option<bollard::Docker> {
    runtime.docker.clone()
}
