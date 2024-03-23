use crate::lemmy_api_common::{
    community::{ListCommunities, ListCommunitiesResponse},
    utils::{apply_label_read, blocking, check_private_instance, get_local_user_view_from_jwt_opt},
};
use crate::lemmy_api_crud::PerformCrud;
use crate::lemmy_db_schema::traits::DeleteableOrRemoveable;
use crate::lemmy_db_views_actor::community_view::CommunityQueryBuilder;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use actix_web::web::Data;

#[async_trait::async_trait(?Send)]
impl PerformCrud for ListCommunities {
    type Response = ListCommunitiesResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "community-list", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<ListCommunitiesResponse, LemmyError> {
        let data: &ListCommunities = self;
        let local_user_view =
            get_local_user_view_from_jwt_opt(data.auth.as_ref(), context.pool(), context.secret())
                .await?;

        apply_label_read(check_private_instance(&local_user_view, context.pool()).await?);

        let person_id = local_user_view.to_owned().map(|l| l.person.id);

        // Don't show NSFW by default
        let show_nsfw = match &local_user_view {
            Some(uv) => uv.local_user.show_nsfw,
            None => false,
        };

        let sort = data.sort;
        let listing_type = data.type_;
        let page = data.page;
        let limit = data.limit;
        let mut communities = apply_label_read(
            blocking(context.pool(), move |conn| {
                CommunityQueryBuilder::create(conn)
                    .listing_type(listing_type)
                    .sort(sort)
                    .show_nsfw(show_nsfw)
                    .my_person_id(person_id)
                    .page(page)
                    .limit(limit)
                    .list()
            })
            .await??,
        );

        // Blank out deleted or removed info for non-logged in users
        if person_id.is_none() {
            for cv in communities
                .iter_mut()
                .filter(|cv| cv.community.deleted || cv.community.removed)
            {
                cv.community = cv.to_owned().community.blank_out_deleted_or_removed_info();
            }
        }

        // Return the jwt
        Ok(ListCommunitiesResponse { communities })
    }
}
