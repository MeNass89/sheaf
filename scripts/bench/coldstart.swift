// usage: coldstart <app path> <window owner name>
// Launches the app with /usr/bin/open and polls the on-screen window list every 50 ms
// until a layer-0 window owned by <owner> exists. Prints seconds from launch.
import CoreGraphics
import Foundation
let app = CommandLine.arguments[1], owner = CommandLine.arguments[2]
func hasWindow() -> Bool {
    let l = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
    return l.contains { ($0["kCGWindowOwnerName"] as? String) == owner && ($0["kCGWindowLayer"] as? Int) == 0 }
}
let t0 = Date()
let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/bin/open"); p.arguments = ["-a", app]
try! p.run()
while !hasWindow() {
    if Date().timeIntervalSince(t0) > 60 { print("timeout"); exit(1) }
    usleep(50_000)
}
print(String(format: "%.2f", Date().timeIntervalSince(t0)))
