extern crate reqwest;

use hyper::header::{HeaderMap,HeaderName,HeaderValue};
use hyper::http::Method;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{thread, time, fmt, fs};
use std::error::Error;

use clap::{Arg, App};

#[derive(Deserialize, Debug)]
struct Config {
    cloudflare_api: CloudflareAPI,

    #[serde(default)]
    cron: bool,
}

#[derive(Deserialize, Debug)]
struct CloudflareAPI {
    auth_email: String,
    api_key: String,
    zone_id: String,
    dns_record_name: String,
}

impl CloudflareAPI {
    fn call_api(&self, url: &String, method: Method, body: Option<&String>) -> Result<String, Box<dyn Error>> {
        let client = Client::new();

        let mut map = HeaderMap::new();
        map.insert(HeaderName::from_static("x-auth-email"), HeaderValue::from_str(&self.auth_email).unwrap());
        map.insert(HeaderName::from_static("x-auth-key"), HeaderValue::from_str(&self.api_key).unwrap());
        map.insert(HeaderName::from_static("content-type"), HeaderValue::from_static("application/json"));

        let new_body: String;
        match body {
            Some(add_body) => {
                new_body = add_body.to_string();
            },
            None => {
                new_body = String::from("");
            },
        }

        Ok(client.request(method, url).headers(map).body(new_body).send()?.text()?)
    }
}

#[derive(Deserialize, Debug, Clone)]
struct CloudflareDNSRecord {
    id: String,

    #[serde(rename(deserialize = "type"))]
    cf_type: String,

    name: String,
    content: String,
}

impl Default for CloudflareDNSRecord{
    fn default () -> CloudflareDNSRecord {
        CloudflareDNSRecord{id: "".to_string(), cf_type: "".to_string(), name:"".to_string(), content:"".to_string()}
    }
}

#[derive(Deserialize, Debug)]
struct CloudflareResp {
    result: Option<Vec<CloudflareDNSRecord>>,
    result_info: Option<CloudflareResultInfo>,
    success: bool,
    errors: Vec<CloudflareError>,
    //messages: String,
}

#[derive(Deserialize, Debug)]
struct CloudflareRespSingle {
    result: Option<CloudflareDNSRecord>,
    result_info: Option<CloudflareResultInfo>,
    success: bool,
    errors: Vec<CloudflareError>,
    //messages: String,
}

#[derive(Deserialize, Debug)]
struct CloudflareResultInfo {
    count: i64,
    total_count: i64,
}

#[derive(Deserialize, Debug)]
struct CloudflareError {
    code: i64,
    message: String,
}

#[derive(Debug)]
struct PublicIPError {
    msg: String
}

impl PublicIPError {
    fn new(msg: &str) -> PublicIPError {
        PublicIPError{msg: msg.to_string()}
    }
}

impl fmt::Display for PublicIPError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.msg)
    }
}

impl Error for PublicIPError {
    fn description(&self) -> &str {
        &self.msg
    }
}

fn get_public_ip() -> Result<String, Box<dyn Error>> {
    let urls_to_try = ["http://icanhazip.com", "http://myexternalip.com/raw", "http://ifconfig.me/ip"];
    let client = Client::new();

    for &url in urls_to_try.iter() {
        //client.get(url).send()?.text()?.trim();
        //Ok(String::from(resp.trim()))

        match client.get(url).send() {
            Ok(resp) => {
                //println!("Response is: {}", resp.status().as_str());
                match resp.text() {
                    Ok(content) => {
                        //println!("Content is: {}", content.trim());
                        return Ok(String::from(content.trim()))
                    },
                    Err(err) => println!("Content is: {}", err),
                }
            }
            Err(err) => println!("Error: {}", err)
        }
    }

    Err(Box::new(PublicIPError::new("Error getting public ip.")))
}

fn get_dns_record_info(config: &CloudflareAPI) -> Result<CloudflareDNSRecord, Box<dyn Error>>{    
    let url = "https://api.cloudflare.com/client/v4/zones/".to_owned() + &config.zone_id + "/dns_records?type=A&name=" + &config.dns_record_name;
    
    let resp = config.call_api(&url, Method::GET, None)?;
    let content: CloudflareResp = serde_json::from_str(&resp)?;
    match content.result {
        Some(records) => Ok(records[0].clone()),
        None => Ok(CloudflareDNSRecord::default()),
    }
}

fn update_dns_record(config: &CloudflareAPI, record_id: String, new_ip: String) -> Result<(), Box<dyn Error>> {
    let url = "https://api.cloudflare.com/client/v4/zones/".to_owned() + &config.zone_id + "/dns_records/" + &record_id;
    let body: String = "{\"content\": \"".to_owned() + &new_ip + "\"}";
    
    config.call_api(&url, Method::PATCH, Some(&body))?;
    println!("Update success.");
    Ok(())
}

// fn read_config() -> Result<Config, Box<dyn Error>> {

// }

fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("Cloudflare Dynamic DNS")
        .about("Small program for dynamic DNS using Clouldflare.")
        .arg(Arg::with_name("config")
                 .value_name("config")
                 .short("c")
                 .long("config")
                 .takes_value(true)
                 .help("Location of config file"))
        .arg(Arg::with_name("auth-email")
                 .value_name("auth-email")
                 .env("CLOUDFLARE_AUTH_EMAIL")
                 .short("e")
                 .long("auth-email")
                 .required_unless("config")
                 .takes_value(true)
                 .help("Auth email for Cloudflare"))
        .arg(Arg::with_name("api-key")
                 .value_name("api-key")
                 .env("CLOUDFLARE_API_KEY")
                 .short("k")
                 .long("api-key")
                 .required_unless("config")
                 .takes_value(true)
                 .help("API key for Cloudflare"))
        .arg(Arg::with_name("zone-id")
                 .value_name("zone-id")
                 .env("CLOUDFLARE_ZONE_ID")
                 .short("z")
                 .long("zone-id")
                 .required_unless("config")
                 .takes_value(true)
                 .help("Zone ID that contains the DNS record for Cloudflare"))
        .arg(Arg::with_name("record-name")
                 .value_name("record-name")
                 .env("CLOUDFLARE_RECORD_NAME")
                 .short("n")
                 .long("record-name")
                 .required_unless("config")
                 .takes_value(true)
                 .help("DNS record name for Cloudflare"))
        .arg(Arg::with_name("cron")
                 .value_name("cron")
                 .env("CLOUDFLARE_CRON")
                 .short("r")
                 .long("cron")
                 .default_value("false")
                 .takes_value(true)
                 .help("Run only once and don't loop."))
        .get_matches();
    
    let config_file: Config;
    match matches.value_of("config") {
        Some(path) => {
            let config_file_contents = fs::read_to_string(path).expect("Could not read file!");
            config_file = serde_json::from_str(&config_file_contents)?;
        },
        None =>  {
            config_file = Config{
                cloudflare_api: CloudflareAPI{
                    auth_email:  matches.value_of("auth-email").unwrap().to_string(),
                    api_key: matches.value_of("api-key").unwrap().to_string(),
                    zone_id: matches.value_of("zone-id").unwrap().to_string(),
                    dns_record_name: matches.value_of("record-name").unwrap().to_string(),
                },
                cron: matches.value_of("cron").unwrap().parse().unwrap(),
            };
        },
    }

    while {
        let public_ip = get_public_ip()?;
        println!("Public IP: ({})", public_ip);

        let cloudflare_record = get_dns_record_info(&config_file.cloudflare_api)?;
        let cloudflare_ip = cloudflare_record.content.clone();
        println!("Cloudflare IP: ({})", cloudflare_ip);

        if public_ip == cloudflare_ip {
            println!("Cloudflare does not need updated.");
        } else {
            println!("Cloudflare needs updated.");
            update_dns_record(&config_file.cloudflare_api, cloudflare_record.id, public_ip)?;
        }

        thread::sleep(time::Duration::from_secs(5));

        //This returns a bool to the while loop, therefore acting like a do-while
        !config_file.cron
    } {}

    Ok(())
}