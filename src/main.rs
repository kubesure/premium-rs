use chrono::{Datelike, Local, NaiveDate};
use log::{error, info};
use redis::{Commands, Connection, RedisError, RedisResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;
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

#[derive(Debug, Error)]
#[non_exhaustive]
enum PremiumError {
    #[error("Internal server error")]
    InternalServer,
    #[error("Invalid request")]
    InvalidInput,
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("Cannot calculate risk for input")]
    RiskCalculation,
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
    let request: HealthRequest;
    match validate_parse_request(&mut req).await {
        Ok(result) => request = result,
        Err(err) => return Ok(handle_error(err)),
    };

    let health_response = calculate_premium(request).await;
    match health_response {
        Ok(premium) => {
            let response: HealthResponse = HealthResponse { premium };
            Ok(make_response(&response)?)
        }
        Err(err) => Ok(handle_error(err)),
    }
}

fn handle_error(err: PremiumError) -> Response {
    match err {
        PremiumError::InternalServer => match make_json_error_response("001", err.to_string()) {
            Ok(response) => response,
            Err(_) => Response::new(StatusCode::InternalServerError),
        },
        PremiumError::InvalidInput => match make_json_error_response("002", err.to_string()) {
            Ok(response) => response,
            Err(_) => Response::new(StatusCode::InternalServerError),
        },

        PremiumError::RiskCalculation => match make_json_error_response("004", err.to_string()) {
            Ok(response) => response,
            Err(_) => Response::new(StatusCode::InternalServerError),
        },
        PremiumError::InvalidHeader(header) => {
            match make_json_error_response(
                "003",
                format!("Header {} not provided or invalid", header),
            ) {
                Ok(response) => response,
                Err(_) => Response::new(StatusCode::InternalServerError),
            }
        }
    }
}

async fn validate_parse_request(
    req: &mut Request<()>,
) -> anyhow::Result<HealthRequest, PremiumError> {
    validate_request(&req)?;
    let body = body_string(req).await?;
    let result = serde_json::from_str::<HealthRequest>(body.as_str());
    match result {
        Ok(request) => Ok(request),
        Err(err) => {
            error!(
                "Serialization error while converting json to struct {}",
                err.to_string()
            );
            Err(PremiumError::InvalidInput)
        }
    }
}
fn validate_request(request: &Request<()>) -> anyhow::Result<Response, PremiumError> {
    validate_headers(request)
}

fn validate_headers(request: &Request<()>) -> anyhow::Result<Response, PremiumError> {
    let content_type = request.header("Content-Type").map(|header| header.as_str());
    match content_type {
        Some("application/json") => Ok(Response::new(StatusCode::Ok)),
        _ => Err(PremiumError::InvalidHeader("content-type".to_string())),
    }
}

async fn body_string(req: &mut Request<()>) -> anyhow::Result<String, PremiumError> {
    let body_result = req.body_string().await;
    match body_result {
        Ok(body) => Ok(body),
        Err(err) => {
            error!("Parsing error of request body {}", err.to_string());
            Err(PremiumError::InternalServer)
        }
    }
}

async fn calculate_premium(input: HealthRequest) -> anyhow::Result<String, PremiumError> {
    let mut premium: String = "0".to_string();
    let age = calculate_age(&input.date_of_birth);
    let score = calculate_score(age);
    info!("age {} score {}", score, age);

    let redis_result = redis_premium(input, score).await;

    match redis_result {
        Ok(values) => {
            for value in values {
                premium = value;
            }
            info!("premium {}", premium);
            Ok(premium)
        }
        Err(err) => Err(err),
    }
}

fn calculate_age(dob_str: &String) -> i32 {
    let result = NaiveDate::parse_from_str(dob_str, "%Y-%m-%d");

    match result {
        Ok(date) => {
            let current_year = Local::now();
            let mut years = current_year.year() - date.year();
            if current_year.day() < date.day() {
                years -= 1;
            }
            info!("years calculated {:?}", years);
            years
        }
        Err(_) => 0,
    }
}

fn calculate_score(age: i32) -> i32 {
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

async fn redis_premium(
    input: HealthRequest,
    score: i32,
) -> anyhow::Result<Vec<String>, PremiumError> {
    let mut conn = match conn_read().await {
        Ok(connection) => connection,
        Err(err) => {
            error!("Redis error while getting connection {}", err.to_string());
            return Err(PremiumError::InternalServer);
        }
    };

    let key = input.code + ":" + input.sum_insured.as_str();
    let result: RedisResult<Vec<String>> = conn.zrangebyscore(key, score, score);
    match result {
        Ok(values) => {
            if values.len() > 1 {
                error!("redis has more than two values for sum assumes and score");
                return Err(PremiumError::RiskCalculation);
            }
            Ok(values)
        }
        Err(err) => {
            error!("Redis error while getting score {}", err.to_string());
            Err(PremiumError::InternalServer)
        }
    }
}

async fn conn_read() -> anyhow::Result<Connection, RedisError> {
    let client = redis::Client::open("redis://localhost:6379");

    match client {
        Ok(client) => {
            let conn = client.get_connection();
            Ok(conn?)
        }
        Err(err) => {
            error!("Redis erorr {}", err.to_string());
            Err(err)
        }
    }
}

fn make_json_error_response(err_code: &str, message: String) -> tide::Result {
    let err = ErrorResponse {
        code: err_code.to_string(),
        message: message.to_string(),
    };
    make_response(&err)
}

fn make_response<T: Serialize>(response: &T) -> tide::Result {
    let data = Body::from_json(&response);
    match data {
        Ok(data) => {
            let mut response = Response::new(StatusCode::Ok);
            response.set_body(data);
            Ok(response)
        }
        Err(err) => {
            info!("Error while converting response {:?}", err);
            Err(tide::Error::from_str(
                StatusCode::InternalServerError,
                "Internal server error",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;

    #[test]
    fn test_calculate_age() {
        let dob_str = String::from("1977-09-14");
        let age = calculate_age(&dob_str);
        assert_eq!(age, 46, "want value 45 got {}", age);
    }

    #[test]
    fn test_calculate_premium() {
        let request: HealthRequest = HealthRequest {
            code: "1A".to_string(),
            sum_insured: "100000".to_string(),
            date_of_birth: "1977-09-14".to_string(),
        };

        task::block_on(async {
            let premium = calculate_premium(request).await;
            assert!(premium.is_ok());
            assert_eq!(premium.unwrap(), "750".to_string());
        });
    }
}
