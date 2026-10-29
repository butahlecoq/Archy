use archy_cli::BoundProjectRead;
use archy_mcp::{
    bounds::{DepthGuard, ResponseGuard},
    framing::{FrameWriter, JsonLineFilter},
    server::ArchyServer,
};
use rmcp::{
    RoleServer, ServiceExt,
    transport::{async_rw::AsyncRwTransport, io::stdio_pipes},
};
use std::path::Path;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || args[1] != "--project" {
        eprintln!("usage: archy-mcp --project FILE");
        std::process::exit(2);
    }
    let project = match BoundProjectRead::open(Path::new(&args[2])) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("project: {:?}", e.code);
            std::process::exit(1);
        }
    };
    let (reader, writer) = stdio_pipes();
    let output = FrameWriter::new(writer);
    let input = JsonLineFilter::new(DepthGuard::new(reader), output.clone());
    let transport =
        AsyncRwTransport::<RoleServer, _, _>::new_server(input, ResponseGuard::new(output))
            .with_max_line_length(1_048_576);
    match ArchyServer::new(project).serve(transport).await {
        Ok(service) => {
            let _ = service.waiting().await;
        }
        Err(_) => {
            eprintln!("MCP transport startup failed");
            std::process::exit(1);
        }
    }
}
