# run these on the file of commit: 852cc1c13861577d411e230ffc8c943e59c2bab8
# %s/\[\n    {\n *"syntax": "\(.*\)",\n *"de.*": "\(.*\)",\n *"format": "\(.*\) format",\n *"code": "\(.*\)"\n *}\n *\]/{\r    "format": "\3",\r    "variants": [\r[27;5;106~   {\r         "operands": "\1",\r         "description": "\2",\r         "code": "\4"\r       }\r    ]\r  }
# %s/\[\n *{\n.*: \(.*\)\n *\(.*\)\n\(.*\): "\(.*\) fo.*\n *\(.*\)\n *},\n.*\n.*: \(.*\)\n *\(.*\)\n.*\n *\(.*\)\n.*\n.*/{\r    "format": "\4",\r    "variants": [\r {\r        "operands": \1\r        \2\r        \5\r      },\r      {\r        "operands": \6\r        \7\r        \8\r      }\r    ]\r  },
# %s/"operands": "[^ ]*\(.*\)",/"operands": [\1 ],

# then run the sed script
# sed "/operands/ { s/,$//;s/\([^\[]\) \]/\1, ]/;s/\[ /[ ,/;s/,\([^, ]*\),/\"\1\"#,/;s/,\([^, ]*\),/\"\1\"#,/;s/,\([^, ]*\),/\"\1\"#,/;s/,//g;s/#/, /g;s/,  / /;s/$/,/ }" resources/instructions.json
# /operands/ { s/,$// s/\([^\[]\) \]/\1, ]/ }
