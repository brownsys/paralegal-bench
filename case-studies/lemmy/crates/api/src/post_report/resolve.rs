use crate::lemmy_api_common::{
    post::{PostReportResponse, ResolvePostReport},
    utils::{
        apply_label_community_write, apply_label_read, blocking, get_local_user_view_from_jwt,
        is_mod_or_admin,
    },
};
use crate::lemmy_db_schema::{source::post_report::PostReport, traits::Reportable};
use crate::lemmy_db_views::structs::PostReportView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::{messages::SendModRoomMessage, LemmyContext, UserOperation};
use crate::Perform;
use actix_web::web::Data;

/// Resolves or unresolves a post report and notifies the moderators of the community
#[async_trait::async_trait(?Send)]
impl Perform for ResolvePostReport {
    type Response = PostReportResponse;

    #[tracing::instrument(skip(context, websocket_id))]
    #[cfg_attr(feature = "post-report-resolve", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        websocket_id: Option<ConnectionId>,
    ) -> Result<PostReportResponse, LemmyError> {
        let data: &ResolvePostReport = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let report_id = data.report_id;
        let person_id = local_user_view.person.id;
        let report = apply_label_read(
            blocking(context.pool(), move |conn| {
                PostReportView::read(conn, report_id, person_id)
            })
            .await??,
        );

        let person_id = local_user_view.person.id;
        is_mod_or_admin(context.pool(), person_id, report.community.id).await?;

        let resolved = data.resolved;
        let resolve_fun = move |conn: &'_ _| {
            if resolved {
                PostReport::resolve(conn, report_id, person_id)
            } else {
                PostReport::unresolve(conn, report_id, person_id)
            }
        };

        if #[cfg(feature = "hypothetical-fix")] {
            let post_report_view = apply_label_read(
                blocking(context.pool(), move |conn| {
                    PostReportView::read(conn, report_id, person_id)
                })
                .await??,
            );
            check_community_ban(
                local_user_view.person.id,
                post_report_view.community.id,
                context.pool(),
            )
            .await?;
            check_community_deleted_or_removed(post_report_view.community_id, context.pool()).await?;
        }

        apply_label_community_write(blocking(context.pool(), resolve_fun).await?)
            .map_err(|e| LemmyError::from_error_message(e, "couldnt_resolve_report"))?;

        let post_report_view = apply_label_read(
            blocking(context.pool(), move |conn| {
                PostReportView::read(conn, report_id, person_id)
            })
            .await??,
        );

        let res = PostReportResponse { post_report_view };

        context.chat_server().do_send(SendModRoomMessage {
            op: UserOperation::ResolvePostReport,
            response: res.clone(),
            community_id: report.community.id,
            websocket_id,
        });

        Ok(res)
    }
}
