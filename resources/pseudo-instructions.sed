/pattern/ {
    s/.*"pattern": ".* \(.*\)",/\1,/
    # s/\([^,]*\),/"\1", /g
    # s/, $//
    s/.*/      "operands": [ \0 ],/

    # operands are:
    s/label(\$t2)//g
    s/label+100000(\$t2)//g
    s/label+100000//g
    s/100000//g # 32 bit signed immediate
    s/-100//g # 16 bit signed immediate
    s/100//g # 16 bit unsigned immediate
    s/10//g # 5 bit immediate, unsigned
    s/100000(\$t2)//g
    s/100(\$t2)//g
    s/(\$t2)//g
    s/label//g
    s/target//g # only for real instructions (j and jal); not in pseudo instruction
    s/\$t1//g
    s/\$t2//g
    s/\$t3//g
    s/\$f1//g
    s/\$f2//g

    # s/\[ ,* \]/"found 'em"/

}

# Remove broff12&dbnop
# Broff12 means:
# branch down 1 line if delayed branching is enabled and do a nop,
# otherwise jump over the nop
/BROFF12"/{
    N
    s/BROFF12",\n *"DBNOP/1/
}

# Compact means:
# before the compact is the general translation
# after the compact comes a translation that works with 16 bit immediates
