require 'ripper'
require 'json'

tree = Ripper.sexp(File.read(ARGV.fetch(0)))
abort 'Cannot parse installed Homebrew formula' unless tree
values = {}
strings = lambda do |node|
  next [] unless node.is_a?(Array)
  next [node[1]] if node[0] == :@tstring_content
  node.flat_map { |child| strings.call(child) }
end
walk = lambda do |node|
  next unless node.is_a?(Array)
  if node[0] == :command && node[1].is_a?(Array)
    name = node[1][1]
    if %w[url sha256 license].include?(name) && !values.key?(name)
      values[name] = strings.call(node[2])
    end
  end
  node.each { |child| walk.call(child) }
end
walk.call(tree)
puts JSON.generate(values)
