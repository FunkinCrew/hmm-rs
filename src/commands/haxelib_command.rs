use std::path::PathBuf;

use anyhow::{anyhow, Ok, Result};
use reqwest::blocking::Client;

use crate::{
    commands,
    hmm::{
        self,
        dependencies::Dependancies,
        haxelib::{Haxelib, HaxelibType},
    },
};

pub fn install_haxelib(
    name: &str,
    version: &Option<String>,
    mut deps: Dependancies,
    json_path: PathBuf,
) -> Result<()> {
    let mut haxelib_install = Haxelib {
        name: name.to_string(),
        haxelib_type: HaxelibType::Haxelib,
        vcs_ref: None,
        dir: None,
        path: None,
        url: None,
        version: None,
    };
    match version {
        Some(version) => haxelib_install.version = Some(version.to_string()),

        None => {
            // we need to query the latest version from haxelib
            // haxelib url: lib.haxe.org/api/3.0/index.n/
            // needs X-Haxe-Remoting header
            // and __x param with the query
            // in __x param, we can query with something like
            // ay3:apiy16:getLatestVersionhay4:limeh

            let serialized = format!("ay3:apiy16:getLatestVersionhay{}:{}h", name.len(), name);
            let client = Client::new();

            let resp = client
                .get("https://lib.haxe.org/api/3.0/index.n/")
                .header("X-Haxe-Remoting", "1")
                .query(&[("__x", serialized)])
                .send()?;

            let resp = resp.text()?;
            let resp_splits = resp.split(":").collect::<Vec<&str>>();
            let decoded_resp = urlencoding::decode(resp_splits[1])?;

            println!("Latest version of {} is {}", name, decoded_resp);

            if decoded_resp.starts_with("No such Project") {
                return Err(anyhow!("{}", decoded_resp)); // this haxelib doesn't exist
            }

            haxelib_install.version = Some(decoded_resp.to_string());
        }
    };
    commands::install_command::install_from_haxelib(&haxelib_install)?;
    deps.dependencies.push(haxelib_install);
    hmm::json::save_json(deps, json_path)?;
    Ok(())
}
