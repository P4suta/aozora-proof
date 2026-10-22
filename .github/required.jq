def required:
  . as $needs |
  .changes.result == "success" and
  (["rust", "web"] | all(.[];
    ($needs.changes.outputs[.] == "true" or $needs.changes.outputs[.] == "false") and
    $needs[.].result == (if $needs.changes.outputs[.] == "true" then "success" else "skipped" end)));

def assurance:
  .coverage.result == "success" and .fuzz.result == "success";

def security:
  {changes: {result: .changes.result, outputs: {rust: .changes.outputs.rust, web: "false"}},
   rust: .scan, web: {result: "skipped"}} | required;
