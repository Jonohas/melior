using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Database.Migrations
{
    /// <inheritdoc />
    public partial class AddSessions : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "sessions",
                schema: "melior",
                columns: table => new
                {
                    id = table.Column<Guid>(type: "uuid", nullable: false),
                    display_name = table.Column<string>(type: "text", nullable: false),
                    channel_id = table.Column<Guid>(type: "uuid", nullable: false),
                    is_muted = table.Column<bool>(type: "boolean", nullable: false),
                    is_deafened = table.Column<bool>(type: "boolean", nullable: false),
                    is_video_enabled = table.Column<bool>(type: "boolean", nullable: false),
                    is_joined_at = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false),
                    last_active_at = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false),
                    is_active = table.Column<bool>(type: "boolean", nullable: false),
                    channel_id1 = table.Column<Guid>(type: "uuid", nullable: true),
                    created_at = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("pk_sessions", x => x.id);
                    table.ForeignKey(
                        name: "fk_sessions_channels_channel_id",
                        column: x => x.channel_id,
                        principalSchema: "melior",
                        principalTable: "channels",
                        principalColumn: "id",
                        onDelete: ReferentialAction.Cascade);
                    table.ForeignKey(
                        name: "fk_sessions_channels_channel_id1",
                        column: x => x.channel_id1,
                        principalSchema: "melior",
                        principalTable: "channels",
                        principalColumn: "id");
                });

            migrationBuilder.CreateIndex(
                name: "ix_sessions_channel_id",
                schema: "melior",
                table: "sessions",
                column: "channel_id");

            migrationBuilder.CreateIndex(
                name: "ix_sessions_channel_id1",
                schema: "melior",
                table: "sessions",
                column: "channel_id1");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "sessions",
                schema: "melior");
        }
    }
}
