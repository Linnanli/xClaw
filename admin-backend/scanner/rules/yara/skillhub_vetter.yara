//////////////////////////////////////////
// Skill-Vetter RED FLAGS - YARA rules
// Source: clawhub.ai/spclaudehome/skill-vetter
//////////////////////////////////////////

rule vetter_agent_memory_theft {

    meta:
        author = "SkillHub (derived from skill-vetter)"
        description = "Detects skills that read agent memory, identity, or personality files to steal context or impersonate the agent"
        classification = "harmful"
        threat_type = "AGENT MEMORY THEFT"

    strings:
        $memory_md    = "MEMORY.md" nocase
        $user_md      = "USER.md" nocase
        $soul_md      = "SOUL.md" nocase
        $identity_md  = "IDENTITY.md" nocase

        $claude_memory   = ".claude/memory" nocase
        $claude_settings = ".claude/settings" nocase
        $claude_config   = "claude_desktop_config.json" nocase

        $open_call  = /\b(open|read|cat|head|tail)\s*\(/
        $path_read  = /Path\s*\([^)]+\)\.(read_text|read_bytes)/

        $doc_ref = /(README|CHANGELOG|CONTRIBUTING|LICENSE)/i

    condition:
        not $doc_ref and
        (
            (
                ($memory_md or $user_md or $soul_md or $identity_md) and
                ($open_call or $path_read)
            )
            or
            $claude_memory or
            $claude_settings or
            $claude_config
        )
}

rule vetter_ip_exfiltration {

    meta:
        author = "SkillHub (derived from skill-vetter)"
        description = "Detects network calls to raw IP addresses instead of domain names, which may bypass DNS logging and content filtering"
        classification = "harmful"
        threat_type = "IP-BASED EXFILTRATION"

    strings:
        $http_ip = /https?:\/\/\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}/
        $socket_ip = /connect\s*\(\s*\(?\s*['\"]\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}/
        $curl_ip = /\b(curl|wget)\s+[^\n]*https?:\/\/\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}/

        $private_10     = /https?:\/\/10\.\d{1,3}\.\d{1,3}\.\d{1,3}/
        $private_172    = /https?:\/\/172\.(1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}/
        $private_192    = /https?:\/\/192\.168\.\d{1,3}\.\d{1,3}/
        $loopback       = /https?:\/\/127\.0\.0\.1/
        $any_addr       = /https?:\/\/0\.0\.0\.0/
        $doc_comment    = /^(\s*#|\s*\/\/|\s*\*)/

    condition:
        not $private_10 and
        not $private_172 and
        not $private_192 and
        not $loopback and
        not $any_addr and
        not $doc_comment and
        (
            $http_ip or
            $socket_ip or
            $curl_ip
        )
}

rule vetter_browser_data_theft {

    meta:
        author = "SkillHub (derived from skill-vetter)"
        description = "Detects skills that access browser cookies, sessions, saved passwords, or profile data"
        classification = "harmful"
        threat_type = "BROWSER DATA THEFT"

    strings:
        $chrome_path   = /Google\/Chrome\/(Default|Profile)/ nocase
        $firefox_path  = /\.mozilla\/firefox\/[^\s]*profiles/ nocase
        $brave_path    = "BraveSoftware" nocase
        $chromium_path = /Chromium\/(Default|Profile)/ nocase
        $edge_path     = "Microsoft/Edge" nocase

        $mac_chrome = "Library/Application Support/Google/Chrome" nocase

        $cookies_db      = "Cookies" nocase
        $login_data      = "Login Data" nocase
        $web_data        = "Web Data" nocase
        $local_storage   = "Local Storage" nocase
        $session_storage = "Session Storage" nocase

        $sqlite_cookies = /sqlite3[^\n]*(Cookies|Login Data|Web Data)/i

        $set_cookie = /Set-Cookie/i
        $cookie_policy = /cookie[_\s]?policy/i
        $documentation = /(```|README|CHANGELOG)/i

    condition:
        not $set_cookie and
        not $cookie_policy and
        not $documentation and
        (
            $chrome_path or
            $firefox_path or
            $brave_path or
            $chromium_path or
            $edge_path or
            $mac_chrome or
            $sqlite_cookies or
            (
                ($cookies_db or $login_data or $web_data) and
                ($chrome_path or $firefox_path or $mac_chrome or $chromium_path)
            )
        )
}
