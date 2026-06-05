pub const BASH: &str = r#"_spriters() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    opts="--help --quiet --input --output --recursive --project --watch
          --follow-links --force --dry-run --check
          --html --json --json-format --caat --cocos2d
          --less --scss --css-template --less-template --scss-template
          --no-css --no-img --css-vars --css-vars-template --tailwind --dts --format
          --css --img --cachebuster --cachebuster-filename
          --cachebuster-filename-only-sprites --separator --namespace
          --sprite-namespace --pseudo-class-separator --url
          --algorithm --ordering --png8 --ratios --retina --crop
          --exclude --max-width --max-height --resize-algorithm
          --padding --margin --no-duplicate"

    case "${prev}" in
        --algorithm|-a)
            COMPREPLY=($(compgen -W "square vertical horizontal diagonal vertical-right horizontal-bottom" -- "${cur}"))
            return 0 ;;
        --ordering)
            COMPREPLY=($(compgen -W "maxside -maxside width -width height -height area -area filename -filename" -- "${cur}"))
            return 0 ;;
        --format)
            COMPREPLY=($(compgen -W "png webp avif" -- "${cur}"))
            return 0 ;;
        --json-format)
            COMPREPLY=($(compgen -W "array hash" -- "${cur}"))
            return 0 ;;
        --resize-algorithm)
            COMPREPLY=($(compgen -W "nearest triangle catmullrom gaussian lanczos3" -- "${cur}"))
            return 0 ;;
        --input|-i|--output|-o|--css|--img|--css-template|--less-template|--scss-template|--css-vars-template)
            COMPREPLY=($(compgen -d -- "${cur}"))
            return 0 ;;
    esac

    COMPREPLY=($(compgen -W "${opts}" -- "${cur}"))
    return 0
}

complete -F _spriters spriters
"#;

pub const ZSH: &str = r#"#compdef spriters

_spriters() {
    local -a opts
    opts=(
        '--help[Show help]'
        '--quiet[Suppress output]'
        {-i,--input}'[Input directory]:dir:_files -/'
        {-o,--output}'[Output directory]:dir:_files -/'
        {-r,--recursive}'[Walk directories recursively]'
        '--project[Generate one sprite per subdirectory]'
        '--watch[Watch for changes]'
        '--follow-links[Follow symbolic links]'
        '--force[Force regeneration]'
        '--dry-run[Print what would be generated without writing files]'
        '--check[Exit non-zero if output is missing or stale]'
        '--html[Generate HTML test page]'
        '--json[Generate JSON metadata]'
        '--json-format[JSON format]:format:(array hash)'
        '--caat[Generate CAAT descriptor]'
        '--cocos2d[Generate Cocos2d XML]'
        {-l,--less}'[Generate LESS stylesheet]'
        '--scss[Generate SCSS stylesheet]'
        '--no-css[Skip CSS generation]'
        '--no-img[Skip image generation]'
        '--css-vars[Generate CSS custom properties file]'
        '--tailwind[Generate Tailwind CSS safelist JSON]'
        '--dts[Generate TypeScript SpriteName type definition]'
        '--format[Sprite image format]:format:(png webp avif)'
        '--cachebuster[Add hash as query param]'
        '--cachebuster-filename[Embed hash in filename]'
        '--cachebuster-filename-only-sprites[Embed hash in sprite filename only]'
        '--separator[CSS class name separator]:sep'
        '--namespace[Global CSS namespace]:ns'
        '--sprite-namespace[Sprite CSS namespace]:ns'
        '--url[Base URL for sprite images]:url'
        {-a,--algorithm}'[Layout algorithm]:algo:(square vertical horizontal diagonal vertical-right horizontal-bottom)'
        '--ordering[Sort order]:order:(maxside -maxside width -width height -height area -area filename -filename)'
        '--png8[Grayscale output]'
        '--ratios[Comma-separated ratios]:ratios'
        '--retina[Shortcut for --ratios=2,1]'
        {-c,--crop}'[Crop transparent borders]'
        '--exclude[Exclude glob pattern]:pattern'
        '--max-width[Maximum image width]:px'
        '--max-height[Maximum image height]:px'
        '--resize-algorithm[Resize algorithm]:algo:(nearest triangle catmullrom gaussian lanczos3)'
        {-p,--padding}'[Cell padding]:px'
        {-m,--margin}'[Cell margin]:px'
        '--no-duplicate[Error on duplicate images]'
    )
    _arguments -s $opts
}

_spriters
"#;

pub const FISH: &str = r#"# fish completion for spriters

complete -c spriters -s h -l help        -d 'Show help'
complete -c spriters -s q -l quiet       -d 'Suppress output'
complete -c spriters -s i -l input       -d 'Input directory'        -r -a '(__fish_complete_directories)'
complete -c spriters -s o -l output      -d 'Output directory'       -r -a '(__fish_complete_directories)'
complete -c spriters -s r -l recursive   -d 'Walk directories recursively'
complete -c spriters      -l project     -d 'One sprite per subdirectory'
complete -c spriters      -l watch       -d 'Watch for changes'
complete -c spriters      -l follow-links -d 'Follow symbolic links'
complete -c spriters      -l force       -d 'Force regeneration'
complete -c spriters      -l dry-run     -d 'Print what would be generated'
complete -c spriters      -l check       -d 'Exit non-zero if output is stale'
complete -c spriters      -l html        -d 'Generate HTML test page'
complete -c spriters      -l json        -d 'Generate JSON metadata'
complete -c spriters      -l json-format -d 'JSON format'              -r -a 'array hash'
complete -c spriters      -l caat        -d 'Generate CAAT descriptor'
complete -c spriters      -l cocos2d     -d 'Generate Cocos2d XML'
complete -c spriters -s l -l less        -d 'Generate LESS stylesheet'
complete -c spriters      -l scss        -d 'Generate SCSS stylesheet'
complete -c spriters      -l no-css      -d 'Skip CSS generation'
complete -c spriters      -l no-img      -d 'Skip image generation'
complete -c spriters      -l css-vars    -d 'Generate CSS custom properties'
complete -c spriters      -l tailwind    -d 'Generate Tailwind CSS safelist JSON'
complete -c spriters      -l dts         -d 'Generate TypeScript SpriteName type'
complete -c spriters      -l format      -d 'Image format'             -r -a 'png webp avif'
complete -c spriters      -l cachebuster -d 'Add hash as query param'
complete -c spriters      -l cachebuster-filename -d 'Embed hash in filename'
complete -c spriters      -l separator   -d 'CSS class name separator' -r
complete -c spriters      -l namespace   -d 'Global CSS namespace'     -r
complete -c spriters      -l url         -d 'Base URL for images'      -r
complete -c spriters -s a -l algorithm   -d 'Layout algorithm'         -r -a 'square vertical horizontal diagonal vertical-right horizontal-bottom'
complete -c spriters      -l ordering    -d 'Sort order'               -r -a 'maxside -maxside width -width height -height area -area filename -filename'
complete -c spriters      -l png8        -d 'Grayscale output'
complete -c spriters      -l ratios      -d 'Comma-separated ratios'   -r
complete -c spriters      -l retina      -d 'Shortcut for --ratios=2,1'
complete -c spriters -s c -l crop        -d 'Crop transparent borders'
complete -c spriters      -l exclude     -d 'Exclude glob pattern'     -r
complete -c spriters      -l max-width   -d 'Maximum image width'      -r
complete -c spriters      -l max-height  -d 'Maximum image height'     -r
complete -c spriters      -l resize-algorithm -d 'Resize filter'       -r -a 'nearest triangle catmullrom gaussian lanczos3'
complete -c spriters -s p -l padding     -d 'Cell padding'             -r
complete -c spriters -s m -l margin      -d 'Cell margin'              -r
complete -c spriters      -l no-duplicate -d 'Error on duplicate images'
"#;
