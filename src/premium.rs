use std::path::Path;

use calamine::{open_workbook_auto, Reader, Xlsx};
use chrono::{Datelike, Local, NaiveDate};
use log::{error, info};
use redis::{Commands, Connection, RedisError, RedisResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct HealthRequest {
    code: String,
    #[serde(rename = "sumInsured")]
    sum_insured: String,
    #[serde(rename = "dateOfBirth")]
    date_of_birth: String,
}

#[derive(Serialize, Debug)]
pub struct HealthResponse {
    pub premium: String,
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PremiumError {
    #[error("Internal server error")]
    InternalServer,
    #[error("Invalid request")]
    InvalidInput,
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("Cannot calculate risk for input")]
    RiskCalculation,
}

pub async fn calculate_premium(input: HealthRequest) -> anyhow::Result<String, PremiumError> {
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
    drop(conn);
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

pub fn load() -> anyhow::Result<bool, PremiumError> {
    let path = "./premium_tables.xlsx";
    let mut work_book = match open_workbook_auto(Path::new(path)) {
        Ok(book) => book,
        Err(_) => return Err(PremiumError::InternalServer),
    };

    let result = work_book.worksheet_range("matrix");

    Ok(true)
}

pub async fn keys_exists() -> anyhow::Result<bool, PremiumError> {
    let mut conn = match conn_read().await {
        Ok(connection) => connection,
        Err(err) => {
            error!("Redis error while getting connection {}", err.to_string());
            return Err(PremiumError::InternalServer);
        }
    };

    let result: Result<Vec<String>, RedisError> = conn.keys("*".to_string());
    drop(conn);
    match result {
        Ok(keys) => {
            if keys.len() > 0 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        Err(_) => Err(PremiumError::InternalServer),
    }
}

async fn unload() -> anyhow::Result<bool, PremiumError> {
    let mut conn = match conn_read().await {
        Ok(connection) => connection,
        Err(err) => {
            error!("Redis error while getting connection {}", err.to_string());
            return Err(PremiumError::InternalServer);
        }
    };

    let result: Result<(), RedisError> = redis::cmd("FLUSHALL").query(&mut conn);
    drop(conn);
    match result {
        Ok(_) => Ok(true),
        Err(_) => Err(PremiumError::InternalServer),
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

    #[test]
    fn test_key_exists() {
        task::block_on(async {
            let result = keys_exists().await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), true);
        });
    }
}
