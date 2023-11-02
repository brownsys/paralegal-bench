use crate::lemmy_api_common::{
    request::purge_image_from_pictrs,
    site::{PurgeItemResponse, PurgePerson},
    utils::{
        apply_label_read, apply_label_write, blocking, get_local_user_view_from_jwt, is_admin,
        purge_image_posts_for_person,
    },
};
use crate::lemmy_db_schema::{
    source::{
        moderator::{AdminPurgePerson, AdminPurgePersonForm},
        person::Person,
    },
    traits::Crud,
};
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use crate::Perform;
use actix_web::web::Data;

#[async_trait::async_trait(?Send)]
impl Perform for PurgePerson {
    type Response = PurgeItemResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "purge-person", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<Self::Response, LemmyError> {
        let data: &Self = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        // Only let admins purge an item
        is_admin(&local_user_view)?;

        // Read the person to get their images
        let person_id = data.person_id;
        let person = apply_label_read(
            blocking(context.pool(), move |conn| Person::read(conn, person_id)).await??,
        );

        if let Some(banner) = person.banner {
            apply_label_write(
                purge_image_from_pictrs(context.client(), context.settings(), &banner)
                    .await
                    .ok(),
            );
        }

        if let Some(avatar) = person.avatar {
            apply_label_write(
                purge_image_from_pictrs(context.client(), context.settings(), &avatar)
                    .await
                    .ok(),
            );
        }

        apply_label_write(
            purge_image_posts_for_person(
                person_id,
                context.pool(),
                context.settings(),
                context.client(),
            )
            .await?,
        );

        apply_label_write(
            blocking(context.pool(), move |conn| Person::delete(conn, person_id)).await??,
        );

        // Mod tables
        let reason = data.reason.to_owned();
        let form = AdminPurgePersonForm {
            admin_person_id: local_user_view.person.id,
            reason,
        };

        apply_label_write(
            blocking(context.pool(), move |conn| {
                AdminPurgePerson::create(conn, &form)
            })
            .await??,
        );

        Ok(PurgeItemResponse { success: true })
    }
}
