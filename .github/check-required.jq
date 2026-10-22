include "required";

def jobs: ["changes", "rust", "web"];
def baseline($rust; $web): {
  changes: {result: "success", outputs: {rust: $rust, web: $web}},
  rust: {result: (if $rust == "true" then "success" else "skipped" end)},
  web: {result: (if $web == "true" then "success" else "skipped" end)}
};
def cases:
  ("false", "true") as $rust | ("false", "true") as $web |
  baseline($rust; $web) as $base | jobs[] as $job |
  ("success", "failure", "cancelled", "skipped") as $result |
  {needs: ($base | .[$job].result = $result), expected: ($result == $base[$job].result)};
def report(verdict):
  reduce cases as $case ({total: 0, matched: 0};
    .total += 1 |
    .matched += (if ($case.needs | verdict) == $case.expected then 1 else 0 end));
def invalid:
  null, 0, "invalid", [],
  (del(.changes.outputs)),
  (jobs[] as $job | del(.[$job]), (.[$job] = null), (.[$job].result = "unknown")),
  (["rust", "web"][] as $job | [null, true, false, 0, "", "TRUE", [], {}][] as $value | .changes.outputs[$job] = $value);
report(required) as $actual |
report(true) as $approve_all |
report(false) as $reject_all |
report(all(.[]; .result == "success" or .result == "skipped")) as $allow_skips |
baseline("true"; "true") |
[invalid | (try required catch false) == false] as $invalid |
[ ("success", "failure", "cancelled", "skipped") as $coverage |
  ("success", "failure", "cancelled", "skipped") as $fuzz |
  {coverage: {result: $coverage}, fuzz: {result: $fuzz}} |
  assurance == ($coverage == "success" and $fuzz == "success") ] as $assurance |
[ ("false", "true") as $rust | ("success", "failure", "cancelled", "skipped") as $result |
  {changes: {result: "success", outputs: {rust: $rust}}, scan: {result: $result}} |
  security == ($result == (if $rust == "true" then "success" else "skipped" end)) ] as $security |
if $actual == {total: 48, matched: 48} and
   $approve_all.matched < 48 and $reject_all.matched < 48 and $allow_skips.matched < 48 and
   ($invalid | length) > 0 and ($invalid | all) and ($assurance | length) == 16 and ($assurance | all) and
   ($security | length) == 8 and ($security | all)
then {cases: $actual.total, assurance_cases: ($assurance | length), security_cases: ($security | length), invalid: ($invalid | length), rejecting_controls: 3}
else error("required-job contract failed")
end
