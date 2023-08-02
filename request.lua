local json = require("json")

function request()
    local payload = {
        code = "1A",
        sumInsured = "100000",
        dateOfBirth = "1990-06-07"
    }
    local body = json.encode(payload)
    wrk.headers["Content-Type"] = "application/json"
    wrk.headers["proxy_http_version"] = "1.1"
    wrk.headers["Content-Length"] = #body
    return wrk.format("POST", nil, nil, body)
end
