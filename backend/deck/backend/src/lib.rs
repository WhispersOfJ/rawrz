//! Library target so integration tests (and the future embedded frontend) can
//! drive the router directly.

pub mod audit;
pub mod catalog;
pub mod credentials;
pub mod docker_helpers;
pub mod env;
pub mod error;
pub mod guards;
pub mod jobs;
pub mod plex;
pub mod probes;
pub mod routes;
pub mod ws;
