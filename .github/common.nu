
# A helper function to display the command being executed.
#
# This also prints the elapsed time the command took to execute.
export def --wrapped run-cmd [...cmd: string] {
    let app = if (
        ($cmd | first) == "cargo"
        or ($cmd | first) == "yarn"
        or ($cmd | first) == 'git'
        or ($cmd | first) == 'gh'
    ) {
        ($cmd | first 2) | str join ' '
    } else if (($cmd | first) == "uv") {
        mut sub_cmd = $cmd.1
        if ($sub_cmd == "run") {
            mut index = 2
            mut skip_val = false
            for arg in ($cmd | skip 2) {
                if ($arg | str starts-with "-") {
                    $skip_val = true
                } else if $skip_val {
                    $skip_val = false
                } else {
                    break
                }
                $index = $index + 1
            }
            if (($cmd | get $index) == "cargo") {
                $sub_cmd = $cmd | skip $index | first 2 | str join ' '
            }
            $sub_cmd
        } else {
            ($cmd | first 2) | str join ' '
        }
    } else {
        ($cmd | first)
    }
    print $"(ansi blue)\nRunning(ansi reset) ($cmd | str join ' ')"
    let elapsed = timeit {|| ^($cmd | first) ...($cmd | skip 1)}
    print $"(ansi magenta)($app) took ($elapsed)(ansi reset)"
}
