use log::info;
use serde::{Deserialize, Serialize};
use tide::{Body, Error, Request, Response, StatusCode};

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
    app.at("/").get(health);
    app.at("/api/v1/healths/premiums").post(premiums);
    let _listener = app.listen("127.0.0.1:8000").await?;
    info!("premium api started");
    Ok(())
}

async fn health(_req: Request<()>) -> tide::Result {
    let response = Response::new(StatusCode::Ok);
    Ok(response)
}

async fn premiums(mut req: Request<()>) -> tide::Result {
    handle_content_type_error(&req).await?;
    let input = req.body_string().await?;
    let result = serde_json::from_str::<HealthRequest>(input.as_str());

    match result {
        Ok(data) => {
            info!("{:?}", data);
            Ok(process_response(data).await)
        }
        Err(err) => {
            info!("{:?}", err);
            Ok(Response::new(tide::StatusCode::InternalServerError))
        }
    }
}

async fn process_response(input: HealthRequest) -> Response {
    let response = HealthResponse {
        premium: "250".to_string(),
    };

    let data = Body::from_json(&response);
    match data {
        Ok(data) => {
            println!("{:?}", response);
            let mut response = Response::new(StatusCode::Ok);
            response.set_body(data);
            response
        }
        Err(err) => {
            info!("Error while converting response {:?}", err);
            Response::new(tide::StatusCode::InternalServerError)
        }
    }
}

async fn handle_content_type_error(request: &Request<()>) -> tide::Result {
    let content_type = request.header("Content-Type").map(|header| header.as_str());
    match content_type {
        Some("application/json") => Ok(Response::new(StatusCode::Ok)),
        _ => Err(handle_request_error(
            "001",
            "content type not application/json".to_string(),
        )),
    }
}

fn handle_request_error(err_code: &str, message: String) -> tide::Error {
    let error_response = ErrorResponse {
        code: err_code.to_string(),
        message: message.to_string(),
    };

    tide::Error::from_str(
        StatusCode::BadRequest,
        serde_json::to_string(&error_response).unwrap(),
    )
}
