def message_callback(commit):
    import re
    msg = commit.message.decode('utf-8', errors='ignore')
    new = re.sub(r'(?i)\bmilo\b', 'cleanup', msg)
    commit.message = new.encode('utf-8')
