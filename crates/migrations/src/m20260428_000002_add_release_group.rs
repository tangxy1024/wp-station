use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("releases", "release_group").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(crate::entity::release::Entity)
                        .add_column(
                            ColumnDef::new(crate::entity::release::Column::ReleaseGroup)
                                .string_len(16)
                                .not_null()
                                .default("models"),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_column("release_targets", "release_group")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(crate::entity::release_target::Entity)
                        .add_column(
                            ColumnDef::new(crate::entity::release_target::Column::ReleaseGroup)
                                .string_len(16)
                                .not_null()
                                .default("models"),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_index("releases", "idx_releases_release_group")
            .await?
        {
            manager
                .create_index(
                    Index::create()
                        .name("idx_releases_release_group")
                        .table(crate::entity::release::Entity)
                        .col(crate::entity::release::Column::ReleaseGroup)
                        .col(crate::entity::release::Column::Status)
                        .col(crate::entity::release::Column::PublishedAt)
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_index("release_targets", "idx_release_targets_prev_success")
            .await?
        {
            manager
                .create_index(
                    Index::create()
                        .name("idx_release_targets_prev_success")
                        .table(crate::entity::release_target::Entity)
                        .col(crate::entity::release_target::Column::DeviceId)
                        .col(crate::entity::release_target::Column::ReleaseGroup)
                        .col(crate::entity::release_target::Column::Status)
                        .col(crate::entity::release_target::Column::CompletedAt)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_index("release_targets", "idx_release_targets_prev_success")
            .await?
        {
            manager
                .drop_index(
                    Index::drop()
                        .name("idx_release_targets_prev_success")
                        .table(crate::entity::release_target::Entity)
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_index("releases", "idx_releases_release_group")
            .await?
        {
            manager
                .drop_index(
                    Index::drop()
                        .name("idx_releases_release_group")
                        .table(crate::entity::release::Entity)
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_column("release_targets", "release_group")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(crate::entity::release_target::Entity)
                        .drop_column(crate::entity::release_target::Column::ReleaseGroup)
                        .to_owned(),
                )
                .await?;
        }

        if manager.has_column("releases", "release_group").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(crate::entity::release::Entity)
                        .drop_column(crate::entity::release::Column::ReleaseGroup)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
