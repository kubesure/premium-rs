mod premium_tests;

use anyhow::{Context, Error};
use chrono::{Datelike, Local, NaiveDate};
use log::{error, info};
use redis::{Commands, Connection, RedisError, RedisResult};
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
            let health_response = process_premium_response(data).await;
            match health_response {
                Ok(data) => Ok(make_response(&data)),
                Err(_err) => {
                    //TODO fix this error response
                    let err_response =
                        make_json_error_response("002", "Internal Server Error".to_string());
                    info!("err: {:?} sending error response", err_response);
                    Ok(err_response)
                }
            }
        }
        Err(err) => {
            let err_response = make_json_error_response("002", "Internal Server Error".to_string());
            info!("err: {:?} sending error response", err_response);
            Ok(err_response)
        }
    }
}

async fn process_premium_response(input: HealthRequest) -> anyhow::Result<HealthResponse> {
    //TODO implement default
    let mut response = HealthResponse {
        premium: "0".to_string(),
    };

    let age = calculate_age(&input.date_of_birth)?;
    let score = calculate_score(age);
    info!("score {}", score);

    let redis_result = redis_premium(input, score).await;

    match redis_result {
        Ok(values) => {
            for premium in values {
                info!("premium is {}", premium);
                response = HealthResponse { premium };
            }
            Ok(response)
        }
        Err(err) => {
            //TODO log error
            error!("{}", err.to_string());
            Err(err)
        }
    }
}

async fn redis_premium(input: HealthRequest, score: u32) -> anyhow::Result<Vec<String>> {
    let mut conn = conn_read().await?;

    let key = input.code + ":" + input.sum_insured.as_str();
    info!("key {} score {}", key, score);
    let result: RedisResult<Vec<String>> = conn.zrangebyscore(key, score, score);
    match result {
        Ok(values) => {
            info!("value length {}", values.len());
            if values.len() > 1 {
                let message: String =
                    String::from("redis has more than two values for sum assumes and score");
                let err = Error::msg(message).context("102".to_string());
                //TODO may not be correct
                return Err(err);
            }
            Ok(values)
        }
        Err(_err) => {
            let message: String = String::from(err.to_string());
            let err = Error::msg(message).context("101".to_string());
            Err(err)
        }
    }
}

fn calculate_age(dob_str: &String) -> anyhow::Result<u32> {
    let result = NaiveDate::parse_from_str(dob_str, "%Y-%m-%d");

    match result {
        Ok(date) => {
            let current_year = Local::now();
            let mut years = current_year.year() - date.year();
            if current_year.day() < date.day() {
                years -= 1;
            }
            info!("years calculated {:?}", years);
            Ok(years.try_into().unwrap_or(0))
        }
        Err(_) => Ok(0),
    }
}

fn calculate_score(age: u32) -> u32 {
    if age >= 18 && age <= 35 {
        return 1;
    } else if age >= 36 && age <= 45 {
        return 2;
    } else if age >= 46 && age <= 55 {
        return 3;
    } else if age >= 56 && age <= 60 {
        return 4;
    } else if age >= 61 && age <= 65 {
        return 5;
    } else if age >= 66 && age <= 70 {
        return 6;
    } else if age > 70 {
        return 7;
    }
    0
}

async fn conn_read() -> anyhow::Result<Connection, RedisError> {
    let client = redis::Client::open("redis://localhost:6379");

    match client {
        Ok(client) => {
            let conn = client.get_connection();
            Ok(conn?)
        }
        Err(err) => Err(err),
    }
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
