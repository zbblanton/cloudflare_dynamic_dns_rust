extern crate reqwest;

use hyper::header::{HeaderMap,HeaderName,HeaderValue};
use hyper::http::Method;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{thread, time, fmt, fs};
use std::error::Error;


#[derive(Deserialize, Debug)]
struct Config {
    cloudflare_api: CloudflareAPI,
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

fn main() -> Result<(), Box<dyn Error>>{
    let config_file_contents = fs::read_to_string("config.json")
        .expect("Could not read file!");

    let config_file: Config = serde_json::from_str(&config_file_contents)?;

    loop {
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
    }
}