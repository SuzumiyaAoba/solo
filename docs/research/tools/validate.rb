# frozen_string_literal: true

# This bundle's structural quality gate, not an official OKF certification tool.
# Run: rtk proxy ruby docs/research/tools/validate.rb
require 'date'
require 'digest'
require 'json'
require 'pathname'
require 'set'
require 'time'
require 'uri'
require 'yaml'

root = File.expand_path('..', __dir__)
errors = []
documents = {}
external_urls = Set.new
graph = Hash.new { |h, k| h[k] = Set.new }
link_count = 0
source_count = 0
footnote_count = 0

duplicate_keys = lambda do |node, path|
  if node.is_a?(Psych::Nodes::Mapping)
    keys = node.children.each_slice(2).map(&:first)
    values = keys.select { |key| key.is_a?(Psych::Nodes::Scalar) }.map(&:value)
    values.group_by(&:itself).each do |key, occurrences|
      errors << "#{path}: duplicate YAML key #{key}" if occurrences.length > 1
    end
  end
  if node.respond_to?(:children) && node.children
    node.children.each { |child| duplicate_keys.call(child, path) }
  end
end

timestamp = lambda do |value, path, field|
  unless value.is_a?(String) && value.match?(/T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})\z/)
    errors << "#{path}: #{field} requires an ISO 8601 instant with offset"
    next
  end
  begin
    Time.iso8601(value)
  rescue ArgumentError
    errors << "#{path}: invalid #{field}"
  end
end

resolve = lambda do |target, path|
  if target.match?(/\Ahttps?:\/\//)
    begin
      uri = URI.parse(target)
      errors << "#{path}: invalid external URL #{target}" unless uri.host
      external_urls << target
    rescue URI::InvalidURIError
      errors << "#{path}: malformed external URL #{target}"
    end
    next nil
  end
  next nil if target.start_with?('#', 'mailto:')
  local = URI::DEFAULT_PARSER.unescape(target.split('#', 2).first)
  resolved = if local.start_with?('/')
               File.expand_path(local.delete_prefix('/'), root)
             else
               File.expand_path(local, File.dirname(File.join(root, path)))
             end
  unless resolved == root || resolved.start_with?(root + File::SEPARATOR)
    errors << "#{path}: link escapes bundle #{target}"
    next nil
  end
  resolved = File.join(resolved, 'index.md') if File.directory?(resolved)
  # The report is produced by this run, so it may not exist on the first run.
  unless File.file?(resolved) || resolved == File.join(root, 'validation.json')
    errors << "#{path}: missing local target #{target}"
  end
  Pathname.new(resolved).relative_path_from(Pathname.new(root)).to_s
end

Dir.glob(File.join(root, '**', '*.md')).sort.each do |file|
  path = Pathname.new(file).relative_path_from(Pathname.new(root)).to_s
  text = File.binread(file).force_encoding(Encoding::UTF_8)
  unless text.valid_encoding?
    errors << "#{path}: invalid UTF-8"
    next
  end
  frontmatter = text.match(/\A---\r?\n(.*?)\r?\n---(?:\r?\n|\z)/m)
  metadata = {}
  body = text
  if frontmatter
    begin
      duplicate_keys.call(Psych.parse(frontmatter[1]), path)
      metadata = YAML.safe_load(frontmatter[1], permitted_classes: [Date], aliases: false)
      raise TypeError, 'frontmatter must be a mapping' unless metadata.is_a?(Hash)
      body = text[frontmatter.end(0)..]
    rescue Psych::Exception, TypeError => e
      errors << "#{path}: #{e.message}"
      next
    end
  end

  name = File.basename(path)
  case name
  when 'index.md'
    if path == 'index.md'
      errors << 'index.md: expected only okf_version: "0.2"' unless metadata == { 'okf_version' => '0.2' }
    elsif frontmatter
      errors << "#{path}: nested index cannot have frontmatter"
    end
    errors << "#{path}: missing index heading" unless body.match?(/^# .+/)
    errors << "#{path}: missing described index entries" unless body.match?(/^[*-] \[[^\n]+\]\([^\n]+\) - .+/)
  when 'log.md'
    errors << "#{path}: log cannot have frontmatter" if frontmatter
    dates = body.scan(/^## (.+)$/).flatten
    errors << "#{path}: missing date sections" if dates.empty?
    dates.each do |date|
      begin
        raise ArgumentError unless date.match?(/\A\d{4}-\d{2}-\d{2}\z/)
        Date.iso8601(date)
      rescue ArgumentError
        errors << "#{path}: invalid log date #{date}"
      end
    end
    errors << "#{path}: log dates must be newest first" unless dates == dates.sort.reverse
  else
    errors << "#{path}: missing frontmatter" unless frontmatter
    %w[type title description].each do |key|
      errors << "#{path}: missing nonempty #{key}" unless metadata[key].is_a?(String) && !metadata[key].strip.empty?
    end
    errors << "#{path}: unexpected okf_version outside root index" if metadata.key?('okf_version')
    errors << "#{path}: empty body" if body.strip.empty?
    generated = metadata['generated']
    if generated.is_a?(Hash)
      errors << "#{path}: missing generated.by" unless generated['by'].is_a?(String) && !generated['by'].empty?
      timestamp.call(generated['at'], path, 'generated.at')
    else
      errors << "#{path}: missing generated mapping"
    end
    timestamp.call(metadata['stale_after'], path, 'stale_after')
    errors << "#{path}: draft status expected before human review" unless metadata['status'] == 'draft'
    errors << "#{path}: do not claim semantic verification from structural checks" if metadata.key?('verified')
    sources = metadata['sources']
    unless sources.is_a?(Array) && !sources.empty?
      errors << "#{path}: sources must be a nonempty list"
      sources = []
    end
    ids = []
    sources.each do |source|
      unless source.is_a?(Hash) && source['resource'].is_a?(String) && !source['resource'].empty?
        errors << "#{path}: invalid source entry"
        next
      end
      id = source['id']
      errors << "#{path}: source needs a stable id" unless id.is_a?(String) && !id.empty?
      ids << id
      target = resolve.call(source['resource'], path)
      graph[path] << target if target&.end_with?('.md')
      source_count += 1
    end
    errors << "#{path}: duplicate source ids" unless ids.uniq == ids
    definitions = body.scan(/^\[\^([a-zA-Z0-9_-]+)\]:/).flatten
    usage_body = body.gsub(/^\[\^[a-zA-Z0-9_-]+\]:[^\n]*$/, '')
    used = usage_body.scan(/\[\^([a-zA-Z0-9_-]+)\]/).flatten.uniq
    errors << "#{path}: duplicate footnote definitions" unless definitions.uniq == definitions
    (used - definitions).each { |id| errors << "#{path}: undefined footnote #{id}" }
    (definitions - used).each { |id| errors << "#{path}: unused footnote #{id}" }
    (used - ids).each { |id| errors << "#{path}: footnote lacks source #{id}" }
    (ids - used).each { |id| errors << "#{path}: source not attributed in body #{id}" }
    footnote_count += used.length
  end

  fences = body.lines.count { |line| line.start_with?('```') }
  errors << "#{path}: unbalanced code fences" unless fences.even?
  body.scan(/(?<!!)\[[^\]\n]+\]\(([^\s)]+)(?:\s+"[^"]*")?\)/).flatten.each do |href|
    target = resolve.call(href, path)
    graph[path] << target if target&.end_with?('.md')
    link_count += 1
  end
  documents[path] = { metadata: metadata, body: body, sha256: Digest::SHA256.hexdigest(text) }
end

documents.keys.select { |path| File.basename(path) == 'index.md' }.each do |path|
  directory = File.dirname(File.join(root, path))
  Dir.children(directory).sort.each do |name|
    next if %w[index.md log.md].include?(name)
    full = File.join(directory, name)
    expected = if File.file?(full) && name.end_with?('.md')
                 full
               elsif File.directory?(full) && !Dir.glob(File.join(full, '**', '*.md')).empty?
                 File.join(full, 'index.md')
               end
    next unless expected
    relative = Pathname.new(expected).relative_path_from(Pathname.new(root)).to_s
    errors << "#{path}: index missing #{relative}" unless graph[path].include?(relative)
  end
end

visited = Set.new
pending = ['index.md']
until pending.empty?
  current = pending.shift
  next if visited.include?(current)
  visited << current
  pending.concat(graph[current].to_a)
end
(documents.keys - visited.to_a).each { |path| errors << "#{path}: unreachable from root index" }

concepts = documents.reject { |path, _| %w[index.md log.md].include?(File.basename(path)) }
report = {
  okf_version: '0.2',
  checked_at: Time.now.utc.iso8601,
  validator: 'bundle-local-structural-check',
  status: errors.empty? ? 'passed' : 'failed',
  semantic_verification: false,
  external_url_reachability_checked: false,
  product_benchmarks_executed: false,
  markdown_files: documents.length,
  concepts: concepts.length,
  product_profiles: concepts.count { |_, d| d[:metadata]['type'] == 'Research Profile' },
  unique_external_urls: external_urls.length,
  source_entries: source_count,
  attributed_footnotes: footnote_count,
  markdown_links: link_count,
  utf8_body_characters: documents.values.sum { |d| d[:body].length },
  errors: errors,
  document_sha256: documents.transform_values { |d| d[:sha256] },
  validator_sha256: Digest::SHA256.file(__FILE__).hexdigest
}
File.write(File.join(root, 'validation.json'), JSON.pretty_generate(report) + "\n")
puts JSON.pretty_generate(report.reject { |key, _| %i[document_sha256 validator_sha256].include?(key) })
exit(errors.empty? ? 0 : 1)
