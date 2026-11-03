//! Standalone server (Docker / LAN). Configuration via environment:
//!   PHOTOSEE_ROOTS     `name=dir;name2=dir2`, read-only roots (required)
//!   PHOTOSEE_PORT      listen port (default 8080)
//!   PHOTOSEE_PASSWORD  optional shared password
//!   PHOTOSEE_DATA      settings and thumbnail cache dir (default ./data)
//!   PHOTOSEE_WEB       built web UI dir (default ./web)
//! Wiring is completed in M2; M1 runs the server embedded in the desktop app.

fn main() {
    eprintln!("photosee-server: standalone mode lands in M2");
    std::process::exit(2);
}
