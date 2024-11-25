use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use socketioxide::{
    extract::{Data, Extension, SocketRef, State},
    SocketIo,
};
use std::sync::{atomic::AtomicUsize, Arc};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

pub struct ChatInitializer;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(transparent)]
struct Username(String);

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", untagged)]
enum Res {
    Login {
        #[serde(rename = "numUsers")]
        num_users: usize,
    },
    UserEvent {
        #[serde(rename = "numUsers")]
        num_users: usize,
        username: Username,
    },
    Message {
        username: Username,
        message: String,
    },
    Username {
        username: Username,
    },
}

#[derive(Clone)]
struct UserCnt(Arc<AtomicUsize>);
impl UserCnt {
    fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }
    fn add_user(&self) -> usize {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }
    fn remove_user(&self) -> usize {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1
    }
}

#[async_trait]
impl Initializer for ChatInitializer {
    fn name(&self) -> String {
        "axum-session".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        let (layer, io) = SocketIo::builder().with_state(UserCnt::new()).build_layer();

        io.ns("/", |s: SocketRef| {
            s.on(
                "new message",
                |s: SocketRef, Data::<String>(msg), Extension::<Username>(username)| {
                    let msg = &Res::Message {
                        username,
                        message: msg,
                    };
                    s.broadcast().emit("new message", msg).ok();
                },
            );

            s.on(
                "add user",
                |s: SocketRef, Data::<String>(username), user_cnt: State<UserCnt>| {
                    if s.extensions.get::<Username>().is_some() {
                        return;
                    }
                    let num_users = user_cnt.add_user();
                    s.extensions.insert(Username(username.clone()));
                    s.emit("login", &Res::Login { num_users }).ok();

                    let res = &Res::UserEvent {
                        num_users,
                        username: Username(username),
                    };
                    s.broadcast().emit("user joined", res).ok();
                },
            );

            s.on("typing", |s: SocketRef, Extension::<Username>(username)| {
                s.broadcast()
                    .emit("typing", &Res::Username { username })
                    .ok();
            });

            s.on(
                "stop typing",
                |s: SocketRef, Extension::<Username>(username)| {
                    s.broadcast()
                        .emit("stop typing", &Res::Username { username })
                        .ok();
                },
            );

            s.on_disconnect(
                |s: SocketRef, user_cnt: State<UserCnt>, Extension::<Username>(username)| {
                    let num_users = user_cnt.remove_user();
                    let res = &Res::UserEvent {
                        num_users,
                        username,
                    };
                    s.broadcast().emit("user left", res).ok();
                },
            );
        });

        let router = router.layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .layer(layer),
        );

        Ok(router)
    }
}
