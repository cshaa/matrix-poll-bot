extern crate dotenv;

use dotenv::dotenv;
use std::env;

use std::time::Duration;

use matrix_sdk::{
    config::SyncSettings,
    ruma::{
        events::{
            poll::{
                unstable_end::UnstablePollEndEventContent,
                unstable_response::UnstablePollResponseEventContent,
                unstable_start::UnstablePollStartEventContent,
            },
            room::member::StrippedRoomMemberEvent,
            AnySyncMessageLikeEvent, AnySyncTimelineEvent, OriginalSyncMessageLikeEvent,
            SyncMessageLikeEvent,
        },
        OwnedUserId, UserId,
    },
    Client, Room,
};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let user =
        OwnedUserId::from(<&UserId>::try_from(env::var("MATRIX_USER").unwrap().as_str()).unwrap());

    let client = Client::builder()
        .server_name(user.server_name())
        .build()
        .await?;

    client
        .matrix_auth()
        .login_username(user, &env::var("MATRIX_PASSWORD").unwrap())
        .send()
        .await?;

    // autojoin new rooms
    client.add_event_handler(
        |room_member: StrippedRoomMemberEvent, client: Client, room: Room| async move {
            if room_member.state_key != client.user_id().unwrap() {
                return;
            }

            tokio::spawn(async move {
                println!("Autojoining room {}", room.room_id());
                let mut delay = 2;

                while let Err(err) = room.join().await {
                    // retry autojoin due to synapse sending invites, before the
                    // invited user can join for more information see
                    // https://github.com/matrix-org/synapse/issues/4345
                    eprintln!(
                        "Failed to join room {} ({err:?}), retrying in {delay}s",
                        room.room_id()
                    );

                    sleep(Duration::from_secs(delay)).await;
                    delay *= 2;

                    if delay > 3600 {
                        eprintln!("Can't join room {} ({err:?})", room.room_id());
                        break;
                    }
                }
                println!("Successfully joined room {}", room.room_id());
            });
        },
    );

    // poll start
    client.add_event_handler(|ev: AnySyncTimelineEvent| async move {
        match ev {
            AnySyncTimelineEvent::MessageLike(x) => match x {
                AnySyncMessageLikeEvent::UnstablePollStart(x) => match x {
                    SyncMessageLikeEvent::Original(x) => handle_poll_start(x),
                    _ => (),
                },
                AnySyncMessageLikeEvent::UnstablePollResponse(x) => match x {
                    SyncMessageLikeEvent::Original(x) => handle_poll_response(x),
                    _ => (),
                },
                AnySyncMessageLikeEvent::UnstablePollEnd(x) => match x {
                    SyncMessageLikeEvent::Original(x) => handle_poll_end(x),
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }
    });

    // This method will never return unless there is an error.
    client.sync(SyncSettings::default()).await?;

    Ok(())
}

fn handle_poll_start(ev: OriginalSyncMessageLikeEvent<UnstablePollStartEventContent>) {
    let content = ev.content.poll_start();
    println!(
        "Started a poll \"{}\" ({}) by {}:",
        content.question.text, ev.event_id, ev.sender,
    );
    for answer in content.answers.iter() {
        println!(" * {} — {}", answer.text, answer.id);
    }
    println!();
}

fn handle_poll_response(ev: OriginalSyncMessageLikeEvent<UnstablePollResponseEventContent>) {
    let content = ev.content.poll_response;
    println!(
        "Response to poll {} by {}:",
        ev.content.relates_to.event_id, ev.sender
    );
    for answer in content.answers.iter() {
        println!("{}", answer);
    }
    println!();
}

fn handle_poll_end(ev: OriginalSyncMessageLikeEvent<UnstablePollEndEventContent>) {
    println!(
        "Ended the poll {} by {}: {}",
        ev.content.relates_to.event_id, ev.sender, ev.content.text
    );
    println!();
}
