use crate::proto_impl;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[clap(name = "ctl")]
pub struct Opt {
    #[command(subcommand)]
    target: Targets,

    #[clap(long = "ctl-target", default_value = "http://localhost:50051")]
    ctl_target: String,
}

#[derive(Subcommand, Debug, Clone)]
enum Targets {
    #[command(subcommand)]
    Conn(OpCmd),
}

#[derive(Subcommand, Debug, Clone)]
enum OpCmd {
    List,
    Kill,
}

pub async fn run(opt: Opt) -> Result<()> {
    let mut client = proto_impl::CtlClient::new(&opt.ctl_target).await?;
    match opt.target {
        Targets::Conn(OpCmd::List) => {
            let res = client.conn_list().await?;
            let mut t = prettytable::Table::new();
            t.set_format(*prettytable::format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
            t.set_titles(prettytable::row!["id", "name", "last_active", "pkt_buf"]);
            res.conns.iter().for_each(|conn| {
                let id = conn.id.clone();
                let name = match conn.name.clone() {
                    Some(name) => name,
                    None => "".to_string(),
                };
                let last_active = match conn.last_active.clone() {
                    Some(last_active) => last_active.to_string(),
                    None => "in_use".to_string(),
                };
                let pkt_buf = match conn.pkt_buf.clone() {
                    Some(pkt_buf) => pkt_buf,
                    None => 0,
                };
                t.add_row(prettytable::row![id, name, last_active, pkt_buf]);
            });
            t.printstd();
        }
        Targets::Conn(OpCmd::Kill) => {
            client.conn_kill("1").await?;
        }
    }
    Ok(())
}
