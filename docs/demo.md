# Creating a demo for the readme

## Install tools

    emerge asciinema libwebp
    git clone https://github.com/zechris/asciinema-rec_script && add to path
    cargo install agg --git https://github.com/asciinema/agg
    cargo install emlop --path .

## Record and convert

    for SCN in *.asc; do
        SCN=${SCN%.asc}
        COMMENT='# ' CAT=cat PROMPT_PAUSE=0 TYPING_PAUSE=0.06 ./$SCN.asc
        sed -i /recording/d $SCN.cast
        agg --cols 116 --rows 20 $SCN.cast $SCN.gif
        gif2webp $SCN.gif -o $SCN.webp
    done
