export function show_error(cmd, str) {
    if (window.damascOutput) {
        window.damascOutput.printError(cmd, str)
    } else {
        console.error(">>"+cmd+"\n"+str)
    }
}

export function show_result(cmd, str) {
    if (window.damascOutput) {
        window.damascOutput.printResult(cmd, str)
    } else {
        console.log(">>"+cmd+"\n"+str)
    }
}