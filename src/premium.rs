use std::env;
use std::path::Path;

use calamine::{open_workbook_auto, Reader};
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
    let age = calculate_age(&input.date_of_birth);
    let score = calculate_score(age);
    info!("age {} score {}", score, age);

    let redis_result = redis_premium(input, score).await;

    match redis_result {
        Ok(values) => Ok(values[0].to_string()),
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
    let mut conn = conn_read().await?;

    let key = input.code + ":" + input.sum_insured.as_str();
    let result: RedisResult<Vec<String>> = conn.zrangebyscore(key, score, score);
    drop(conn);
    match result {
        Ok(values) => {
            if values.len() != 1 {
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

pub async fn load() -> anyhow::Result<bool, PremiumError> {
    let premium_table = load_excel_data().await?;
    let mut conn = conn_read().await?;

    for i in 0..premium_table.len() {
        let mut premium: i32 = 0;
        let mut score: i32 = 0;
        let mut key: String = "".to_string();

        for j in 0..premium_table[i].len() {
            if j == 0 {
                key = premium_table[i][j].to_string();
            } else if j == 1 {
                match premium_table[i][j].parse::<i32>() {
                    Ok(number) => premium = number,
                    Err(_err) => return Err(PremiumError::InternalServer),
                }
            } else if j == 2 {
                match premium_table[i][j].parse::<i32>() {
                    Ok(number) => score = number,
                    Err(_err) => return Err(PremiumError::InternalServer),
                }
            }
        }
        let result: Result<(), RedisError> = conn.zadd(key, premium, score);
        match result {
            Ok(_) => {}
            Err(_) => return Err(PremiumError::InternalServer),
        }
    }
    Ok(true)
}

//
async fn load_excel_data() -> anyhow::Result<Vec<Vec<String>>, PremiumError> {
    let path = "./premium_tables.xlsx";
    let mut work_book = match open_workbook_auto(Path::new(path)) {
        Ok(book) => book,
        Err(_) => return Err(PremiumError::InternalServer),
    };

    if let Some(Ok(range)) = work_book.worksheet_range("matrix") {
        let mut score = 0;

        let mut premim_table: Vec<Vec<String>> = Vec::with_capacity(3);
        for row in range.rows() {
            let mut key = "".to_string();
            score += 1;
            let mut premium_row: Vec<String> = Vec::with_capacity(3);
            for (index, value) in row.iter().enumerate() {
                if index == 0 {
                    key = value.to_string();
                }

                if index == 1 {
                    let colon = String::from(":");
                    key = key + &colon + &value.to_string();
                    premium_row.push(key.to_string());
                }

                if index == 3 {
                    premium_row.push(value.to_string());
                }
            }
            premium_row.push(score.to_string());
            premim_table.push(premium_row);
        }
        Ok(premim_table)
    } else {
        Err(PremiumError::RiskCalculation)
    }
}

pub async fn keys_exists() -> anyhow::Result<bool, PremiumError> {
    let mut conn = conn_read().await?;

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
        Err(err) => {
            error!("Redis error while fetching keys{}", err.to_string());
            Err(PremiumError::InternalServer)
        }
    }
}

pub async fn unload() -> anyhow::Result<bool, PremiumError> {
    let mut conn = conn_read().await?;

    let result: Result<(), RedisError> = redis::cmd("FLUSHALL").query(&mut conn);
    drop(conn);
    match result {
        Ok(_) => Ok(true),
        Err(err) => {
            error!(
                "Redis error while executing command FLUSHALL{}",
                err.to_string()
            );
            Err(PremiumError::InternalServer)
        }
    }
}

impl From<String> for HealthResponse {
    fn from(value: String) -> Self {
        HealthResponse { premium: value }
    }
}

impl Into<String> for HealthResponse {
    fn into(self) -> String {
        self.premium
    }
}

async fn redis_svc() -> anyhow::Result<String, PremiumError> {
    let result = env::var("redissvc");
    match result {
        Ok(value) => Ok(value),
        Err(_) => {
            error!("Error while getting redis service from variable");
            Err(PremiumError::InternalServer)
        }
    }
}

//TODO fix to use read and write as diffrent connections
async fn conn_read() -> anyhow::Result<Connection, PremiumError> {
    //TODO fix to load to static from variable
    let redis_svc = format!("redis://{}:6379", redis_svc().await?);
    info!("redis connection string {}", redis_svc);
    let client = redis::Client::open(redis_svc);

    match client {
        Ok(client) => {
            let conn = client.get_connection();
            match conn {
                Ok(conn) => Ok(conn),
                Err(err) => {
                    error!("Redis connection error {}", err.to_string());
                    Err(PremiumError::InternalServer)
                }
            }
        }
        Err(err) => {
            error!("Redis client opening error {}", err.to_string());
            Err(PremiumError::InternalServer)
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

    #[test]
    fn test_load() {
        task::block_on(async {
            let result = load().await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), true);
        });
    }

    #[test]
    fn test_unload() {
        task::block_on(async {
            let result = unload().await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), true);
        });
    }
}
