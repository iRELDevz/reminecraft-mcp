using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;

var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();

app.MapGet("/health", () => Results.Json(new { ok = true, agent = "windows-agent", version = "0.2.0" }));

app.MapPost("/launch", (LaunchReq req) =>
{
    try
    {
        string file;
        string arguments;
        if (!string.IsNullOrWhiteSpace(req.Executable))
        {
            file = req.Executable;
            arguments = req.Args ?? "";
        }
        else
        {
            if (string.IsNullOrWhiteSpace(req.Instance))
                return Results.BadRequest(new { ok = false, error = "instance atau executable wajib diisi" });
            file = Native.FindPrism();
            if (file == "")
                return Results.Json(new { ok = false, error = "prismlauncher tidak ditemukan, isi executable manual" }, statusCode: 500);
            arguments = "--launch \"" + req.Instance + "\"";
            if (!string.IsNullOrWhiteSpace(req.Args))
                arguments += " " + req.Args;
        }
        var psi = new ProcessStartInfo(file, arguments) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(file) ?? "" };
        var p = Process.Start(psi);
        if (p == null)
            return Results.Json(new { ok = false, error = "Process.Start mengembalikan null" }, statusCode: 500);
        return Results.Json(new { ok = true, pid = p.Id, file, arguments });
    }
    catch (Exception ex)
    {
        return Results.Json(new { ok = false, error = ex.Message }, statusCode: 500);
    }
});

app.MapGet("/screenshot", () =>
{
    try
    {
        var vx = Native.GetSystemMetrics(76);
        var vy = Native.GetSystemMetrics(77);
        var vw = Native.GetSystemMetrics(78);
        var vh = Native.GetSystemMetrics(79);
        using var bmp = new Bitmap(vw, vh);
        using (var g = Graphics.FromImage(bmp))
            g.CopyFromScreen(vx, vy, 0, 0, new Size(vw, vh));
        using var ms = new MemoryStream();
        bmp.Save(ms, ImageFormat.Png);
        return Results.Json(new { ok = true, width = vw, height = vh, image_base64 = Convert.ToBase64String(ms.ToArray()) });
    }
    catch (Exception ex)
    {
        return Results.Json(new { ok = false, error = ex.Message }, statusCode: 500);
    }
});

app.MapGet("/processes", () =>
{
    var names = new[] { "javaw", "java", "prismlauncher", "tlauncher", "Minecraft", "bedrock_server" };
    var list = new List<object>();
    foreach (var n in names)
    {
        Process[] ps;
        try { ps = Process.GetProcessesByName(n); }
        catch { continue; }
        foreach (var p in ps)
        {
            string title = "";
            try { title = p.MainWindowTitle ?? ""; } catch { }
            list.Add(new { name = p.ProcessName, pid = p.Id, title });
            p.Dispose();
        }
    }
    return Results.Json(new { ok = true, count = list.Count, processes = list });
});

app.Run();

[SupportedOSPlatform("windows")]
static class Native
{
    [DllImport("user32.dll")]
    public static extern int GetSystemMetrics(int nIndex);

    public static string FindPrism()
    {
        var path = Environment.GetEnvironmentVariable("PATH") ?? "";
        foreach (var d in path.Split(Path.PathSeparator))
        {
            var f = Path.Combine(d.Trim(), "prismlauncher.exe");
            if (File.Exists(f)) return f;
        }
        var cands = new[] {
            "C:\\Program Files\\PrismLauncher\\prismlauncher.exe",
            "C:\\Program Files (x86)\\PrismLauncher\\prismlauncher.exe",
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs\\PrismLauncher\\prismlauncher.exe"),
        };
        foreach (var f in cands)
            if (File.Exists(f)) return f;
        return "";
    }
}

record LaunchReq(string? Instance, string? Mode, string? Executable, string? Args);
