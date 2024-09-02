(function() {var type_impls = {
"hydroflow_plus":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Sink%3C(u32,+T)%3E-for-DemuxDrain%3CT,+S%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/hydroflow_deploy_integration/lib.rs.html#498\">source</a><a href=\"#impl-Sink%3C(u32,+T)%3E-for-DemuxDrain%3CT,+S%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, S&gt; <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>, T)&gt; for <a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt;<div class=\"where\">where\n    S: <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;T, Error = <a class=\"struct\" href=\"hydroflow_plus/tokio/io/struct.Error.html\" title=\"struct hydroflow_plus::tokio::io::Error\">Error</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle\" open><summary><section id=\"associatedtype.Error\" class=\"associatedtype trait-impl\"><a href=\"#associatedtype.Error\" class=\"anchor\">§</a><h4 class=\"code-header\">type <a href=\"hydroflow_plus/futures/trait.Sink.html#associatedtype.Error\" class=\"associatedtype\">Error</a> = <a class=\"struct\" href=\"hydroflow_plus/tokio/io/struct.Error.html\" title=\"struct hydroflow_plus::tokio::io::Error\">Error</a></h4></section></summary><div class='docblock'>The type of value produced by the sink when an error occurs.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.poll_ready\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/hydroflow_deploy_integration/lib.rs.html#501\">source</a><a href=\"#method.poll_ready\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.poll_ready\" class=\"fn\">poll_ready</a>(\n    self: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html\" title=\"struct core::pin::Pin\">Pin</a>&lt;&amp;mut <a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt;&gt;,\n    _cx: &amp;mut <a class=\"struct\" href=\"hydroflow_plus/futures/task/struct.Context.html\" title=\"struct hydroflow_plus::futures::task::Context\">Context</a>&lt;'_&gt;,\n) -&gt; <a class=\"enum\" href=\"hydroflow_plus/futures/task/enum.Poll.html\" title=\"enum hydroflow_plus::futures::task::Poll\">Poll</a>&lt;<a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.unit.html\">()</a>, &lt;<a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt; as <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>, T)&gt;&gt;::<a class=\"associatedtype\" href=\"hydroflow_plus/futures/trait.Sink.html#associatedtype.Error\" title=\"type hydroflow_plus::futures::Sink::Error\">Error</a>&gt;&gt;</h4></section></summary><div class='docblock'>Attempts to prepare the <code>Sink</code> to receive a value. <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.poll_ready\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.start_send\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/hydroflow_deploy_integration/lib.rs.html#509\">source</a><a href=\"#method.start_send\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.start_send\" class=\"fn\">start_send</a>(\n    self: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html\" title=\"struct core::pin::Pin\">Pin</a>&lt;&amp;mut <a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt;&gt;,\n    item: (<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>, T),\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.unit.html\">()</a>, &lt;<a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt; as <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>, T)&gt;&gt;::<a class=\"associatedtype\" href=\"hydroflow_plus/futures/trait.Sink.html#associatedtype.Error\" title=\"type hydroflow_plus::futures::Sink::Error\">Error</a>&gt;</h4></section></summary><div class='docblock'>Begin the process of sending a value to the sink.\nEach call to this function must be preceded by a successful call to\n<code>poll_ready</code> which returned <code>Poll::Ready(Ok(()))</code>. <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.start_send\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.poll_flush\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/hydroflow_deploy_integration/lib.rs.html#521\">source</a><a href=\"#method.poll_flush\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.poll_flush\" class=\"fn\">poll_flush</a>(\n    self: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html\" title=\"struct core::pin::Pin\">Pin</a>&lt;&amp;mut <a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt;&gt;,\n    _cx: &amp;mut <a class=\"struct\" href=\"hydroflow_plus/futures/task/struct.Context.html\" title=\"struct hydroflow_plus::futures::task::Context\">Context</a>&lt;'_&gt;,\n) -&gt; <a class=\"enum\" href=\"hydroflow_plus/futures/task/enum.Poll.html\" title=\"enum hydroflow_plus::futures::task::Poll\">Poll</a>&lt;<a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.unit.html\">()</a>, &lt;<a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt; as <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>, T)&gt;&gt;::<a class=\"associatedtype\" href=\"hydroflow_plus/futures/trait.Sink.html#associatedtype.Error\" title=\"type hydroflow_plus::futures::Sink::Error\">Error</a>&gt;&gt;</h4></section></summary><div class='docblock'>Flush any remaining output from this sink. <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.poll_flush\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.poll_close\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/hydroflow_deploy_integration/lib.rs.html#529\">source</a><a href=\"#method.poll_close\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.poll_close\" class=\"fn\">poll_close</a>(\n    self: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/pin/struct.Pin.html\" title=\"struct core::pin::Pin\">Pin</a>&lt;&amp;mut <a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt;&gt;,\n    _cx: &amp;mut <a class=\"struct\" href=\"hydroflow_plus/futures/task/struct.Context.html\" title=\"struct hydroflow_plus::futures::task::Context\">Context</a>&lt;'_&gt;,\n) -&gt; <a class=\"enum\" href=\"hydroflow_plus/futures/task/enum.Poll.html\" title=\"enum hydroflow_plus::futures::task::Poll\">Poll</a>&lt;<a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.unit.html\">()</a>, &lt;<a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt; as <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>, T)&gt;&gt;::<a class=\"associatedtype\" href=\"hydroflow_plus/futures/trait.Sink.html#associatedtype.Error\" title=\"type hydroflow_plus::futures::Sink::Error\">Error</a>&gt;&gt;</h4></section></summary><div class='docblock'>Flush any remaining output and close this sink, if necessary. <a href=\"hydroflow_plus/futures/trait.Sink.html#tymethod.poll_close\">Read more</a></div></details></div></details>","Sink<(u32, T)>","hydroflow_plus::util::deploy::BufferedDrain"],["<section id=\"impl-Unpin-for-DemuxDrain%3CT,+S%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/hydroflow_deploy_integration/lib.rs.html#491\">source</a><a href=\"#impl-Unpin-for-DemuxDrain%3CT,+S%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;'pin, T, S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"hydroflow_plus/util/deploy/struct.DemuxDrain.html\" title=\"struct hydroflow_plus::util::deploy::DemuxDrain\">DemuxDrain</a>&lt;T, S&gt;<div class=\"where\">where\n    S: <a class=\"trait\" href=\"hydroflow_plus/futures/trait.Sink.html\" title=\"trait hydroflow_plus::futures::Sink\">Sink</a>&lt;T, Error = <a class=\"struct\" href=\"hydroflow_plus/tokio/io/struct.Error.html\" title=\"struct hydroflow_plus::tokio::io::Error\">Error</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> + ?<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a>,\n    __DemuxDrain&lt;'pin, T, S&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,</div></h3></section>","Unpin","hydroflow_plus::util::deploy::BufferedDrain"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()