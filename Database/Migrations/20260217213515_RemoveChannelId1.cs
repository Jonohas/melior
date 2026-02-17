using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Database.Migrations
{
    /// <inheritdoc />
    public partial class RemoveChannelId1 : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropForeignKey(
                name: "fk_sessions_channels_channel_id1",
                schema: "melior",
                table: "sessions");

            migrationBuilder.DropIndex(
                name: "ix_sessions_channel_id1",
                schema: "melior",
                table: "sessions");

            migrationBuilder.DropColumn(
                name: "channel_id1",
                schema: "melior",
                table: "sessions");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.AddColumn<Guid>(
                name: "channel_id1",
                schema: "melior",
                table: "sessions",
                type: "uuid",
                nullable: true);

            migrationBuilder.CreateIndex(
                name: "ix_sessions_channel_id1",
                schema: "melior",
                table: "sessions",
                column: "channel_id1");

            migrationBuilder.AddForeignKey(
                name: "fk_sessions_channels_channel_id1",
                schema: "melior",
                table: "sessions",
                column: "channel_id1",
                principalSchema: "melior",
                principalTable: "channels",
                principalColumn: "id");
        }
    }
}
