use crate::Perform;
use actix_web::web::Data;
use lemmy_api_common::{
  post::{PostResponse, StickyPost},
  utils::{
    blocking,
    check_community_ban,
    check_community_deleted_or_removed,
    get_local_user_view_from_jwt,
    is_mod_or_admin,
    apply_localuserview_label,
  },
};
use lemmy_apub::{
  objects::post::ApubPost,
  protocol::activities::{create_or_update::post::CreateOrUpdatePost, CreateOrUpdateType},
};
use lemmy_db_schema::{
  source::{
    moderator::{ModStickyPost, ModStickyPostForm},
    post::Post,
  },
  traits::Crud,
};
use lemmy_utils::{error::LemmyError, ConnectionId};
use lemmy_websocket::{send::send_post_ws_message, LemmyContext, UserOperation};

#[dfpp::label(noinline)]
fn apply_post_label(l2 : &Post) -> &Post {
  return l2;
}

#[async_trait::async_trait(?Send)]
impl Perform for StickyPost {
  type Response = PostResponse;

  #[tracing::instrument(skip(context, websocket_id))]
  #[dfpp::analyze]
  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    websocket_id: Option<ConnectionId>,
  ) -> Result<PostResponse, LemmyError> {
    let data: &StickyPost = self;
    let local_user_view_og =
      get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

    let post_id = data.post_id;
    let orig_post_og = blocking(context.pool(), move |conn| Post::read(conn, post_id)).await??;

    let local_user_view = apply_localuserview_label(&local_user_view_og);
    let orig_post = apply_post_label(&orig_post_og);

    check_community_ban(
      local_user_view.person.id,
      orig_post.community_id,
      context.pool(),
    )
    .await?;
    check_community_deleted_or_removed(orig_post.community_id, context.pool()).await?;

    // Verify that only the mods can sticky
    is_mod_or_admin(
      context.pool(),
      local_user_view.person.id,
      orig_post.community_id,
    )
    .await?;

    // Update the post
    let post_id = data.post_id;
    let stickied = data.stickied;
    let updated_post: ApubPost = blocking(context.pool(), move |conn| {
      Post::update_stickied(conn, post_id, stickied)
    })
    .await??
    .into();

    // Mod tables
    let form = ModStickyPostForm {
      mod_person_id: local_user_view.person.id,
      post_id: data.post_id,
      stickied: Some(stickied),
    };
    blocking(context.pool(), move |conn| {
      ModStickyPost::create(conn, &form)
    })
    .await??;

    // Apub updates
    // TODO stickied should pry work like locked for ease of use
    CreateOrUpdatePost::send(
      updated_post,
      &local_user_view.person.clone().into(),
      CreateOrUpdateType::Update,
      context,
    )
    .await?;

    send_post_ws_message(
      data.post_id,
      UserOperation::StickyPost,
      websocket_id,
      Some(local_user_view.person.id),
      context,
    )
    .await
  }
}
