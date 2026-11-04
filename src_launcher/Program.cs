using System;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Threading;

namespace MyMusicLauncher
{
    static class Program
    {
        [STAThread]
        static void Main()
        {
            // 1. Locate root working directory
            string exeDir = AppDomain.CurrentDomain.BaseDirectory;
            string rootDir = exeDir;
            if (!File.Exists(Path.Combine(rootDir, "library.json")))
            {
                // If compiled inside subfolder or target, fall back to current directory
                rootDir = Directory.GetCurrentDirectory();
            }

            // 2. Find Rust Audio Core backend executable
            string rustExe = Path.Combine(rootDir, @"target\debug\my_music_player.exe");
            if (!File.Exists(rustExe))
            {
                rustExe = Path.Combine(rootDir, "my_music_player.exe");
            }

            Process backendProcess = null;

            // 3. Check if server is already running on port 4080
            if (!IsServerLive())
            {
                if (File.Exists(rustExe))
                {
                    try
                    {
                        ProcessStartInfo psi = new ProcessStartInfo
                        {
                            FileName = rustExe,
                            WorkingDirectory = rootDir,
                            CreateNoWindow = true,
                            WindowStyle = ProcessWindowStyle.Hidden,
                            UseShellExecute = false
                        };
                        backendProcess = Process.Start(psi);
                    }
                    catch { }

                    // Wait for backend to be ready (up to 5 seconds)
                    for (int i = 0; i < 25; i++)
                    {
                        Thread.Sleep(200);
                        if (IsServerLive()) break;
                    }
                }
            }

            // 4. Find Chrome or Edge to launch in standalone App mode
            string browserPath = FindBrowserExecutable();
            if (string.IsNullOrEmpty(browserPath))
            {
                // Fallback to default system browser
                Process.Start("http://127.0.0.1:4080/");
                return;
            }

            // Dedicated isolated profile directory
            string profileDir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                @"MyMusicApp\Profile"
            );
            try { Directory.CreateDirectory(profileDir); } catch { }

            string args = string.Format(
                "--app=\"http://127.0.0.1:4080/\" --user-data-dir=\"{0}\" --window-size=1280,820 --app-id=MyMusicHiRes --disable-features=Translate",
                profileDir
            );

            ProcessStartInfo appPsi = new ProcessStartInfo
            {
                FileName = browserPath,
                Arguments = args,
                UseShellExecute = true
            };

            Process appProcess = null;
            try
            {
                appProcess = Process.Start(appPsi);
            }
            catch
            {
                Process.Start("http://127.0.0.1:4080/");
                return;
            }

            // 5. If we have the app process, wait for user to close it, then clean up backend
            if (appProcess != null)
            {
                try
                {
                    appProcess.WaitForExit();
                }
                catch { }

                // Clean up background backend process if we started it
                if (backendProcess != null && !backendProcess.HasExited)
                {
                    try
                    {
                        backendProcess.Kill();
                    }
                    catch { }
                }
            }
        }

        static bool IsServerLive()
        {
            try
            {
                HttpWebRequest req = (HttpWebRequest)WebRequest.Create("http://127.0.0.1:4080/api/status");
                req.Timeout = 400;
                req.Method = "GET";
                using (HttpWebResponse resp = (HttpWebResponse)req.GetResponse())
                {
                    return resp.StatusCode == HttpStatusCode.OK;
                }
            }
            catch
            {
                return false;
            }
        }

        static string FindBrowserExecutable()
        {
            string[] candidates = new string[]
            {
                @"C:\Program Files\Google\Chrome\Application\chrome.exe",
                @"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Google\Chrome\Application\chrome.exe"),
                @"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                @"C:\Program Files\Microsoft\Edge\Application\msedge.exe"
            };

            foreach (string path in candidates)
            {
                if (File.Exists(path)) return path;
            }
            return null;
        }
    }
}
