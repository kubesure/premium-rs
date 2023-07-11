mod premium_tests;

use anyhow::Context;
use chrono::{Datelike, Local, NaiveDate};
use log::info;
use serde::{Deserialize, Serialize};
use tide::{Body, Request, Response, StatusCode};

#[derive(Debug, Deserialize)]
struct HealthRequest {
    code: String,
    #[serde(rename = "sumInsured")]
    sum_insured: String,
    #[serde(rename = "dateOfBirth")]
    date_of_birth: String,
}

#[derive(Serialize, Debug)]
struct HealthResponse {
    premium: String,
}

#[derive(Serialize, Debug)]
struct ErrorResponse {
    code: String,
    message: String,
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let mut app = tide::new();
    app.at("/").get(healthz);
    app.at("/api/v1/healths/premiums").post(premiums);
    let _listener = app.listen("127.0.0.1:8000").await?;
    info!("premium api started");
    Ok(())
}

async fn healthz(_req: Request<()>) -> tide::Result {
    let response = Response::new(StatusCode::Ok);
    Ok(response)
}

async fn premiums(mut req: Request<()>) -> tide::Result {
    validate_request(&req).await?;
    let input = req.body_string().await?;
    let result = serde_json::from_str::<HealthRequest>(input.as_str());

    match result {
        Ok(data) => {
            info!("{:?}", data);
            let health_response = process_premium_response(data).await?;
            Ok(make_response(&health_response))
        }
        Err(err) => {
            info!("{:?}", err);
            Ok(make_json_error_response(
                "002",
                "Internal Server Error".to_string(),
            ))
        }
    }
}

async fn process_premium_response(input: HealthRequest) -> anyhow::Result<HealthResponse> {
    //TODO calculate premium

    let age = calculate_age(&input.date_of_birth)?;

    let response = HealthResponse {
        premium: "250".to_string(),
    };
    Ok(response)
}

fn calculate_age(dob_str: &String) -> anyhow::Result<i32> {
    let result = NaiveDate::parse_from_str(dob_str, "%Y-%m-%d");

    match result {
        Ok(date) => {
            let current_year = Local::now();
            let mut years = current_year.year() - date.year();
            if current_year.day() < date.day() {
                years -= 1;
            }
            info!("years calculated {:?}", years);
            Ok(years)
        }
        Err(_) => Ok(0),
    }
}

fn calculate_score(age: u32) -> anyhow::Result<u32> {
    Ok(12)
}

fn make_json_error_response(err_code: &str, message: String) -> Response {
    let err = ErrorResponse {
        code: err_code.to_string(),
        message: message.to_string(),
    };
    make_response(&err)
}

fn make_response<T: Serialize>(response: &T) -> Response {
    let data = Body::from_json(&response);
    match data {
        Ok(data) => {
            let mut response = Response::new(StatusCode::Ok);
            response.set_body(data);
            response
        }
        Err(err) => {
            info!("Error while converting response {:?}", err);
            Response::new(StatusCode::InternalServerError)
        }
    }
}

async fn validate_request(request: &Request<()>) -> tide::Result {
    let content_type = request.header("Content-Type").map(|header| header.as_str());
    match content_type {
        Some("application/json") => Ok(Response::new(StatusCode::Ok)),
        _ => {
            Err(handle_request_error("001", "content type not application/json".to_string()).await)
        }
    }
}

async fn handle_request_error(err_code: &str, message: String) -> tide::Error {
    let error_response = ErrorResponse {
        code: err_code.to_string(),
        message: message.to_string(),
    };

    tide::Error::from_str(
        StatusCode::BadRequest,
        serde_json::to_string(&error_response).unwrap_or("Error".to_string()),
    )
}
