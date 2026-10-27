//! macOS: Sheaf's tools in Finder's right-click menu (Quick Actions, and the Services menu), as
//! iLovePDF's context menu runs a tool on the PDFs picked (Sol, 27/10: on macOS only the default
//! reader was set). Each tool is a Quick Action of the kind Automator saves, written to the user's
//! `~/Library/Services`: one Run Shell Script step that starts Sheaf with `--tool=<key>` and the
//! files, the same command the Linux desktop entry's actions and Windows' verbs run. Plain files,
//! so they are written and checked on any system; macOS reads them when `pbs -update` is run.

use std::path::{Path, PathBuf};

/// Writes one `<label>.workflow` per tool of [`crate::open_with::MENU_TOOLS`] into `services`
/// (created if needed), each running `exe`. Returns the bundles written.
pub fn install(services: &Path, exe: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut made = Vec::new();
    for (key, label) in crate::open_with::MENU_TOOLS {
        let bundle = services.join(format!("{label}.workflow"));
        let contents = bundle.join("Contents");
        std::fs::create_dir_all(&contents)?;
        std::fs::write(contents.join("Info.plist"), info_plist(label))?;
        std::fs::write(contents.join("document.wflow"), workflow(&command(exe, key)))?;
        made.push(bundle);
    }
    Ok(made)
}

/// The shell command a Quick Action runs: Sheaf with the tool and the files Finder passes as
/// arguments, in the background so Finder's spinner stops at once.
pub fn command(exe: &Path, key: &str) -> String {
    // Single-quoted for /bin/sh; a quote in the path closes, is escaped and reopens.
    let exe = exe.to_string_lossy().replace('\'', r"'\''");
    format!("'{exe}' --tool={key} \"$@\" >/dev/null 2>&1 &")
}

/// XML text: & < > escaped.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

const HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n";

/// The bundle's Info.plist: one service named `label`, shown by Finder on PDFs.
fn info_plist(label: &str) -> String {
    format!(
        "{HEAD}<dict>\n\t<key>NSServices</key>\n\t<array>\n\t\t<dict>\n\t\t\t<key>NSBackgroundColorName</key>\n\t\t\t<string>background</string>\n\t\t\t<key>NSIconName</key>\n\t\t\t<string>NSActionTemplate</string>\n\t\t\t<key>NSMenuItem</key>\n\t\t\t<dict>\n\t\t\t\t<key>default</key>\n\t\t\t\t<string>{}</string>\n\t\t\t</dict>\n\t\t\t<key>NSMessage</key>\n\t\t\t<string>runWorkflowAsService</string>\n\t\t\t<key>NSRequiredContext</key>\n\t\t\t<dict>\n\t\t\t\t<key>NSApplicationIdentifier</key>\n\t\t\t\t<string>com.apple.finder</string>\n\t\t\t</dict>\n\t\t\t<key>NSSendFileTypes</key>\n\t\t\t<array>\n\t\t\t\t<string>com.adobe.pdf</string>\n\t\t\t</array>\n\t\t</dict>\n\t</array>\n</dict>\n</plist>\n",
        xml(label)
    )
}

/// The Automator document: a Quick Action taking PDF files in Finder, its one step Run Shell
/// Script with the files passed as arguments (`inputMethod` 1).
fn workflow(command: &str) -> String {
    let cmd = xml(command);
    format!(
        r#"{HEAD}<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.path</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{cmd}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/sh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>8C1F4D2E-5A3B-4E6F-9D10-2B7C8E9F0A11</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>3E9A7B5C-1D2F-4A6B-8C0D-4F5E6A7B8C22</string>
				<key>UUID</key>
				<string>6B2D8E4F-7A1C-4B3D-9E5F-1A2B3C4D5E33</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict/>
				<key>isViewVisible</key>
				<integer>1</integer>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.PDF</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<integer>0</integer>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject.PDF</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<integer>0</integer>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<integer>0</integer>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    /// Every tool of the menu is a Quick Action bundle whose two property lists parse (Python's
    /// plistlib reads Apple's format), named for Finder on PDFs, running Sheaf with its tool and the
    /// files; a path with a quote and spaces stays one argument to the shell.
    #[test]
    fn each_menu_tool_is_a_quick_action_finder_shows_on_pdfs() {
        let dir = tempfile::tempdir().unwrap();
        let exe = std::path::Path::new("/Applications/Sheaf's apps/Sheaf.app/Contents/MacOS/sheaf");
        let made = super::install(dir.path(), exe).unwrap();
        assert_eq!(made.len(), crate::open_with::MENU_TOOLS.len());
        let check = r#"
import plistlib, sys
info = plistlib.load(open(sys.argv[1] + "/Contents/Info.plist", "rb"))
s = info["NSServices"][0]
assert s["NSMessage"] == "runWorkflowAsService" and s["NSSendFileTypes"] == ["com.adobe.pdf"], s
wf = plistlib.load(open(sys.argv[1] + "/Contents/document.wflow", "rb"))
a = wf["actions"][0]["action"]
assert a["ActionName"] == "Run Shell Script" and a["ActionParameters"]["inputMethod"] == 1
assert wf["workflowMetaData"]["workflowTypeIdentifier"] == "com.apple.Automator.servicesMenu"
print(s["NSMenuItem"]["default"]); print(a["ActionParameters"]["COMMAND_STRING"])
"#;
        let o = std::process::Command::new("python3").args(["-c", check]).arg(dir.path().join("Compress with Sheaf.workflow")).output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let mut lines = out.lines();
        assert_eq!(lines.next(), Some("Compress with Sheaf"));
        let cmd = lines.next().unwrap();
        assert_eq!(cmd, r#"'/Applications/Sheaf'\''s apps/Sheaf.app/Contents/MacOS/sheaf' --tool=compress "$@" >/dev/null 2>&1 &"#);
        // The shell reads it as the program, the tool, and each file as one argument.
        let o = std::process::Command::new("sh").args(["-c", &cmd.replace(" >/dev/null 2>&1 &", "").replacen("'/", "printf '%s|' '/", 1), "sh", "a b.pdf", "c.pdf"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout), "/Applications/Sheaf's apps/Sheaf.app/Contents/MacOS/sheaf|--tool=compress|a b.pdf|c.pdf|");
        for (_, label) in crate::open_with::MENU_TOOLS {
            assert!(dir.path().join(format!("{label}.workflow/Contents/document.wflow")).is_file(), "{label}");
        }
    }
}
