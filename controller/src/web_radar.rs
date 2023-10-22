use actix_web::{middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_ws::Message;
use futures_util::stream::StreamExt;


async fn ws(req: HttpRequest, body: web::Payload) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    actix_rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        return;
                    }
                }
                Message::Text(s) => println!("Got text, {}", s),
                _ => break,
            }
        }

        let _ = session.close(None).await;
    });

    Ok(response)
}

pub async fn run_server() -> Result<(), anyhow::Error> {
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/ws", web::get().to(ws))
    })
        .bind("0.0.0.0:6969")?
        .run()
        .await?;

    Ok(())
}