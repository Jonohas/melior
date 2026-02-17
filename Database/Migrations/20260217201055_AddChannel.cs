using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Database.Migrations
{
    /// <inheritdoc />
    public partial class AddChannel : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(
                name: "melior");

            migrationBuilder.RenameTable(
                name: "todo_items",
                schema: "todos",
                newName: "todo_items",
                newSchema: "melior");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.EnsureSchema(
                name: "todos");

            migrationBuilder.RenameTable(
                name: "todo_items",
                schema: "melior",
                newName: "todo_items",
                newSchema: "todos");
        }
    }
}
