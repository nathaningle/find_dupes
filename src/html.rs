use std::io::Write;

use crate::DedupFile;

pub fn write_dupes_html(dest: &mut impl Write, dupes: &[Vec<DedupFile>]) {
    writeln!(dest, "{}", HTML_TOP).unwrap();
    for group in dupes {
        dedup_group_to_html_tr(dest, group);
    }
    writeln!(dest, "{}", HTML_BOTTOM).unwrap();
}

fn dedup_group_to_html_tr(dest: &mut impl Write, group: &[DedupFile]) {
    write!(dest, "    <tr><td>").unwrap();
    for df in group {
        write!(
            dest,
            "<p><code>{}</code></p>",
            df.paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<String>>()
                .join("</code>, <code>")
        )
        .unwrap();
    }
    writeln!(dest, "</td><td>{}</td></tr>", group[0].size).unwrap();
}

const HTML_TOP: &str = "<!doctype html>
<html lang=\"en\">
  <head>
    <meta charset=\"utf-8\">
    <title>Results</title>
    <style>
        html {
            font-family: sans-serif;
        }

        table {
            border-collapse: collapse;
            border: 1px solid black;
            margin: 1em;
        }

        th, td {
            padding: 0.5em 1em;
            border: 1px solid black;
        }
    </style>
  </head>
  <body>
    <table>
      <thead>
        <tr><th>Files</th><th>Size</th></tr>
      </thead>
      <tbody>";

const HTML_BOTTOM: &str = "</tbody>
    </table>
  </body>
</html>";
