use lambda_http::{lambda, Request, Response, Body, StatusCode};
use serde_json::{json, Value};
use rusoto_core::Region;
use rusoto_dynamodb::{DynamoDb, DynamoDbClient, GetItemInput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    lambda!(handler);
    Ok(())
}

async fn handler(request: Request) -> Result<Response<Body>, lambda_http::Error> {
    let operation = request.method().as_str();
    
    if operation == "OPTIONS" {
        // Return a 200 OK response with CORS headers
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type, X-Amz-Date, Authorization, X-Api-Key")
            .body(Body::Text(json!({"message": "CORS preflight response"}).to_string()))
            .unwrap());
    }
    
    // Parse the incoming JSON payload
    let body = match request.body() {
        Body::Text(body) => body,
        _ => return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::Text("Invalid request body".to_string()))
            .unwrap()),
    };
    
    let parsed_body: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(_) => return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::Text("Invalid JSON payload".to_string()))
            .unwrap()),
    };
    
    let bar_name = parsed_body["barName"].as_str().unwrap_or_default();
    let drink_name = parsed_body["drinkName"].as_str().unwrap_or_default();
    
    // Initialize DynamoDB client
    let client = DynamoDbClient::new(Region::default());
    
    // Query DynamoDB using the Bar Name and Drink Name as keys
    let input = GetItemInput {
        table_name: String::from("drink_images"),
        key: json!({
            "barName": { "S": bar_name },
            "drinkName": { "S": drink_name }
        }),
        ..Default::default()
    };
    
    match client.get_item(input).await {
        Ok(response) => {
            match response.item {
                Some(item) => {
                    // Extract the ObjectURL attribute
                    let object_url = item["s3ObjectKey"]["S"].as_str().unwrap_or_default();
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .body(Body::Text(json!({"s3ObjectKey": object_url}).to_string()))
                        .unwrap())
                },
                None => Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::Text(json!({"message": "Item not found"}).to_string()))
                    .unwrap())
            }
        },
        Err(_) => Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::Text("Internal Server Error".to_string()))
            .unwrap())
    }
}
