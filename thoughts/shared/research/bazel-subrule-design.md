# subrule: Decomposing Starlark rules

*Please read Bazel [Code of Conduct](https://www.contributor-covenant.org/version/1/4/code-of-conduct) before commenting.*

| Authors: ilist, hvdStatus: Draft | In review | Approved | Rejected | In progress | ImplementedReviewers: brandjon (Starlark), jcater (Configurability, LGTMed), nharmata (LGTMed)Created: 2023-01-18Updated: 2023-06-30 Discussion thread: \<link\> |
| :---- |

# Overview

A new API is introduced that supports better architecture of Starlark rules. The API may be incrementally used on existing rulesets.

The need for this API was expressed by the users in [Coding guidelines for Starlark rules](https://docs.google.com/document/d/1uuX1Gz6Kvivp-oOqGja342VrVO534PLF0m_zFtFIBcc/edit#heading=h.5mcn15i0e1ch), where guidelines that would improve the architecture of Starlark rules were proposed.

Problems this design is addressing:

- **reusing parts** of the rule implementation (in particular functions creating actions)  
- **encapsulation** of reusable units:  
  - **implicit deps** are declared/accessed without the outer rule needing to declare them (or rely on some kind of programmatic framework to help declare them)  
  - **toolchain resolution** (and exec groups), reusable units get their own   
- **isolation from outer rule's ctx** \-- no passing the god object along everywhere. With this, we get something of an expectation of robustness that you don't have from pure starlark functions.  
- **type enforcement** for args passed to the reusable units

Those problems have already been addressed by various frameworks/helpers implemented in Starlark. The main benefit of the new API is **its** **simplicity and its reusability** across different rulesets.

A separate doc [Extending Bazel rules](https://docs.google.com/document/d/1p6z-shWf9sdqo_ep7dcjZCGvqN5r2jsPkJCqHHgfRp4/edit#heading=h.5mcn15i0e1ch) addresses:

- **extending existing rules**:   
  - for example Bazel users can extend Bazel Java rules with features that Google doesn’t need  
  - Google can extend Bazel Java rules with features (or “temporary” legacy code, that requires larger depot cleanup)   
- **reusing whole rules to create new rules**  
  - for example pytype rules: [pytype\_impl.bzl](https://source.corp.google.com/piper///depot/google3/devtools/python/blaze/pytype/pytype_impl.bzl;l=1634;bpv=1;bpt=1;rcl=503860835)  
  - py\_extension rule  
- making it easier for some macros to be rewritten as rules.

# Summary

- subrule: a new API for creating building blocks of rules  
- subrules=\[my\_subrule\] parameter is added to rule, aspect and subrule  
- other rules/building blocks are called directly from the implementation functions (not via ctx) 

Example of a building block:

| def \_android\_lint\_call(ctx, source\_files, source\_jars,\*, \_android\_lint\_wrapper):     \# a simple ctx and \_android\_lint\_wrapper are provided     \# Rationale: [ctx.attrs vs parameters](#ctx.attrs-vs-parameters)     \# other parameters (source\_files, source\_jars) can have arbitrary type     args \= ctx.actions.args()     args.add\_all("--sources", source\_files)     args.add\_all("--source\_jars", source\_jars)     android\_lint\_out \= ctx.actions.declare\_file(               "%s\_android\_lint\_output.xml" % ctx.label.name)     ctx.actions.run(         mnemonic \= "AndroidLint",         executable \= \_android\_lint\_wrapper,         inputs \= depset(source\_files \+ source\_jars),         outputs \= \[android\_lint\_out\],     )     return android\_lint\_out  \# return value can have arbitrary type android\_lint\_call \= subrule(   implementation \= \_android\_lint\_call,   attrs \= {        \# The name \_android\_lint\_wrapper is private for this subrule        \# and the name can be used in other subrules without a conflict     "\_android\_lint\_wrapper": attr.label(          default \= "//android\_lint\_wrapper",         executable \= True,         cfg \= "exec",   } )  |
| :---- |

Example of an invocation:

| load(":android\_lint.bzl", "android\_lint\_call") def \_impl(ctx):    android\_lint\_out \= android\_lint\_call(                               source\_files \= ctx.files.srcs, source\_jars \= \[\])    return \[DefaultInfo(files \= depset(\[android\_lint\_out\]))\] java\_library \= rule(   implementation \= \_impl,   attrs \= {     'srcs': attr.label\_list(allow\_files \= \[".java"\])   },   subrules \= \[android\_lint\_call\], ) |
| :---- |

# Migration plan

1. **Builtins:** the new API is first exposed only to builtins  
2. **MVP** API includes subrule without exec\_compatible\_with, exec\_groups and the most basic SubruleContext, subrules parameter on rule and subrule calls  
3. **Java ruleset**: API is first used and verified on the Java ruleset  
   1. For example: java\_common.compile is wrapped subrule  
4. **Experimental flag:** After that we expose it under experimental flag and an allowlist  
5. Do a case study on: py\_extension, pytype rules, AOSP  
6. If we discover the API viable, proceed to other major rulesets: **Proto, C++**

This is a rather careful plan that makes it possible to do quick adjustments and extensions.

# Building blocks

## subrule

| callable subrule(implementation, attrs\=None, toolchains\=\[\], exec\_compatible\_with\=\[\], exec\_group\=None, subrules\=\[\]) |
| :---- |

Creates a new sub rule that can be called from rules or other sub rule’s at analysis time. 

**Export.** A subrule doesn’t need to be assigned to a global variable in a .bzl file, like a rule. This means that a subrule can be assigned directly in a struct or exported via different means.

**Comparison to rule.** Subrule can’t define public attributes and can use parameters on its implementation function to pass anything (types are not limited).

### Parameters

| Parameter | Description |
| :---- | :---- |
| implementation | required The Starlark function implementing this subrule. It must have at least ctx parameter and parameters corresponding to names of implicit dependencies (see attrs parameter). It may have more parameters. It may use \*args or \*\*kwargs. The implementation function is called during the analysis phase.  The first positional parameter is set to SubruleContext (the ctx) and implicit dependencies are passed by keyword. Other parameters are set by the user invoking subrule. It must create actions to generate all the declared outputs. This is enforced by Bazel. Best practice: implementation function is private and next to the subrule. This should prevent users from accidentally using the implementation function directly. Best practice: don’t rename the ctx parameter, it’s short and it’s a subset of RuleContext. |
| attrs | dict; or None; default \= None Dictionary to declare all implicit dependencies of the call. It maps from a dependency name to attr.label or attr.label\_list. Other types of attributes are not allowed. Late bound default label are allowed, but not computed defaults (because we can’t get values of parameters before evaluation and we don’t have public attributes). All attribute names must start with an underscore and are private. This helps distinguish “injected” parameters on the implementation function from others. |
| toolchains | sequence; default \= \[\] Set of toolchains this call requires.  The list can contain String, Label, or StarlarkToolchainTypeApi objects, in any combination.  |
| exec\_compatible\_with	 **(not in MVP)** | sequence of strings; default \= \[\] A list of constraints on the execution platform used for actions created by this call. |
| exec\_group**(not in MVP)** | String; default \= None Execution group name.   |
| ~~doc~~ | Not available; the documentation needs to be in a DocString, because available parameters are not documented in attrs, and it would be weird to describe them in their type definitions. This is different from the documentation of rules and providers. We consider the DocString experience better (from users point of view and tooling that generates docs).  |
| subrules | Explained in the next section. |

Handling collisions in relevant namespaces:

- **Output artifact.** Files generated by actions need to be different.  
  In case of the conflict ActionConflict is reported. The proposed API doesn't handle this in any special way. This is the same as what happens within rules or even across targets in a package.

  - If this becomes a problem, we can consider setting name on the invocations, which should contain the name of the calling target (similarly to macros). Bazel could verify that the names within the rule are unique.

- **Implicit attribute names.** Short/simple names of the implicit attributes may be the same as the names other action functions define. The implementation needs to handle this:  
  - The implementation preferably does this **without name mangling**, that is by using a data structure to hold implicit dependencies of each subrule separately.  
  - Bazel does surface/reveal the names of implicit attributes:  
    - native.existing\_rules \- [StarlarkNativeModule.java](http://google3/third_party/bazel/src/main/java/com/google/devtools/build/lib/packages/StarlarkNativeModule.java;l=670;rcl=508080426)  
    - bazel query \--output={xml,proto} \--xml:default\_values //p:t  
    - bazel info build-language, the feature is rarely used and defunct due to Starlakrification.  
    - bazel query 'attr($stub\_template, .\*, //java\_binary )'  
  - The query shall print out longer names, that also reveal the subrule using particular dependency:

| $ blaze query \--output\=xml \--xml:default\_values //p:t \<?xml version\="1.1" encoding\="UTF-8" standalone\="no"?\> \<query version\="2"\>     \<rule class\="java\_library" location\="p/t/BUILD:100:13" name\="//p:t"\> …         \<label name\="@rules\_java//java\_common/compile.bzl%compile$java\_toolchain" value\="//tools/jdk:current\_java\_toolchain"/\> …     \</rule\> \</query\> |
| :---- |

- **Toolchain types.** Toolchain type can be merged.  
  Merging action functions may result in picking a different execution platform. This will be prevented by [Automatic exec groups for toolchains](https://docs.google.com/document/d/1-rbP_hmKs9D639YWw5F_JyxPxL2bi6dSmmvj_WXak9M/edit) (being already implemented).

- **Execution group names.**  
  - When exec\_compatible\_with is set, subrule needs to automatically create a separate “exec\_group” and apply it to used actions. This supports encapsulation and prevents collision with the use of the exec\_compatible\_with in other subrules.  
  - When exec\_group is set, a global namespace is assumed, to support ”cpp\_link” use case (a globally named exec group is used to set specific execution requirements on specific targets). That exec group definition has exec\_compatible\_with and toolchains set to values prescribed by subrule.  All exec\_groups created in such a way need to be consistent.  
  - Only one exec\_group per subrule is supported. A dict of exec\_groups is not going to be supported, because it can be achieved by further splitting subrule and because dicts seem to be overengineered in combination with subrules.

The most extreme case of handling collisions is using the same subrule twice. This works, unless there is a conflict in output files.

## SubruleContext object

A context object that is passed to the implementation function of a subrule. 

### SubruleContext members (equal to RuleContext):

- **actions** \- all actions created by SubruleContext, implicitly set toolchain or exec\_group parameter. Setting these parameters by the user results in an error. (Not allowing users to set a consistent value, makes it easier to remove those parameters from the actions in the future).   
- **toolchains \-** returns provider from one of the specified toolchains.  
- **label** \- needed to name the artifacts

SubruleContext possible members:

- fragments:  Although we’re favouring Starlark flags, we might consider adding fragments to improve encapsulation, and have symmetry with RuleContext. That is, a subrule might define required fragments that would automatically be collected on the rule, but exposed only to the subrule to read them.   
- configuration.target\_platform\_has\_constraint: We should try to find better ways to make platform independent rules, without using this field.  
- configuration.coverage\_enabled: This will be needed.  
- coverage\_instrumented(target): Implements coverage filter. This will be needed.  
- info\_file, version\_file: Special inputs to the rules. Most likely the rule should keep control over them. Prefer to keep them on the rule-level only and pass them to subrules.  
- runfiles: Constructed from multiple public parameters. Most likely the rule should keep control over them. Prefer to keep them on the rule-level only and pass them to subrules.

**Members that should NEVER be provided:**

| Member | Not provided because: |
| :---- | :---- |
| **attr, file, files, executable** | See: [ctx.attrs vs parameters](#ctx.attrs-vs-parameters) |
| **bin\_dir** | The bin/genfiles path is already exposed on declared artifacts through file.root.path. |
| **exec\_groups** | only one exec group may be used in a subrule |
| rule, aspect\_ids | Aspects may use subrules. In the interest of encapsulation, rule’s public attributes or aspect\_ids shouldn’t be accessed directly. |
| created\_actions, build\_setting\_value | only special uses/rules |
| new\_file, genfiles\_dir, default\_provider,expand\_make\_variables | deprecated |
| resolve\_command | experimental |
| resolve\_tools | this should probably be deprecated (it’s already supported by exec attributes) |
| outputs | outputs can’t be declared by subrule, because they are always public attributes |
| split\_attr | We won’t allow split configuration transitions on implicit deps, to keep the API simple.  (There is a use case of split implicit dependency on android\_binary, which will need to be cleaned up / handled in a different way) |
| features, disabled\_features | Those combine features from configuration, package and rule public attributes. subrule doesn’t have public attributes. |
| expand\_location | Collects labels from public attributes. subrule doesn’t have public attributes. |
| var | It collects variables from some hard coded public attributes and toolchains. This breaks encapsulation. The variables can be directly used from a toolchain / implicit dependency. |
| workspace\_name | Should be deprecated. It’s the name of the main repository, given in the root WORKSPACE file. Rules/targets shouldn’t care about it. |
| build\_file\_path | This should probably be deprecated as it is similar to ctx.label.package. |
| configuration  .bin\_dir,    .genfiles\_dir,  .default\_shell\_env,  .test\_env | Should probably be deprecated. |
| configuration.host\_path\_separator | This doesn't belong in configuration. A better location should be found for both rule and subrule. |

Notes on the SubruleContext:

- **Coding guidelines**. By providing a fresh ctx object to action functions, the following guidelines become obsolete:  
  - §4 Building blocks should have actions as a parameter.  
  - §8 The implementation function shall use ctx only to create actions and access implicit dependencies.   
- There are a lot of uses of bin\_dir and label, that’s why we decided to use a stripped down ctx.  
- We could remove toolchains from SubruleContext if they were provided via implicit dependency, for example attr.toolchain(“//toolchain\_type”). That would make it more similar to other implicit dependencies (and less similar to current mechanisms)  
- **Hacker hat** \- because of minimalty of SubruleContext following actions don’t seem to benefit users:  
  - Passing/Returning SubruleContext to another action function  
  - Passing RuleContext to action function:  
    - possible solution: lock RuleContext while evaluating action function  
  - Obtaining and returning an implicit dependency  
  - Returning SubruleContext in a provider

# Composing action functions

### kTo make the action functions composable, the following parameter is added to rule, aspect and subrule.

| Parameter | Description |
| :---- | :---- |
| subrules | sequence; default \= \[\] A list of subrules. If a subrule is called from the implementation function and not declared, an error is raised. Caveat: The error is raised at runtime, when the subrule is actually called. If a subrule call happens in a code path that’s not evaluated at the time, the error is not raised. On the other hand, there are use-cases where a subrule is declared, but not called in all situations, for example validation actions in some rules are not created in exec configuration. The list of symbols is used to resolve all implicit dependencies at load time. This means that a rule depends on implicit dependencies of all subrules that it declares. |

See alternatives: [ctx.subrules.foo](#ctx.subrules.foo), [No subrules parameter](#no-subrules-parameter)

Example:

| load(":android\_lint.bzl", "android\_lint\_call") def \_impl(ctx):    android\_lint\_out \= android\_lint\_call(                             source\_files \= ctx.files.srcs, source\_jars \= \[\])    return \[DefaultInfo(files \= depset(\[android\_lint\_out\]))\] java\_library \= rule(   implementation \= \_impl,   attrs \= {     'srcs': attr.label\_list(allow\_files \= \[".java"\])   },   subrules \= \[android\_lint\_call\] ) |
| :---- |

## Call semantics of an subrule

Symbol returned by subrule is a function (callable). 

It extends the given arguments with the first position argument set to ctx and adds keyword arguments for implicit dependencies.

It may be called only from another subrule, rule, or aspect implementation.

It may return arbitrary value (not limited to a list of providers).

Notes:

- subrules are only available at analysis time. There is no schizophrenic behaviour, that would happen if rules could call other rules from their implementation functions. That would mean a rule could be called either at loading time (creating a target) or at analysis time (creating something similar to a subrule).  
- There is no “signature inspection”. Existing implementation of call invocation is used. Special patterns like \*args and \*\*kwargs may be used.

## Future extensions

- **Typing.** attrs  may be extended to also include public parameters and describe their types

Hypothetical example of public attrs for subrule (those wouldn’t be connected to the rule’s public attributes in any way):

| attrs \= {    "source\_files": type.list(type.file()),     "source\_jars": type.depset(type.file()),    "\_android\_lint\_wrapper": attr.label(...), } |
| :---- |

- Adding types would provide better debugging and prevent frequent user errors (I’ve seen often passing Target instead of ProtoLangToolchainInfo to proto\_common.compile, it happened to me and others)  
- **Requiring types on all parameters** would limit degrees of freedom, which might be beneficial to prevent bad practices. However, we consider this too restrictive and different from regular Starlark functions.  
- There are 2 levels of typing:  
  - 1\) the rule-level type schema of attributes \-- string, label\_list, etc.   
  - 2\) type system for providers and subrules, e.g. depset\<file\>, or MyOtherInfo instance.  
- Typing is out of scope of this document, and should be analysed comprehensively for rules, provider, macros and other possible locations.  
- **Visibility.** The tools used in subrule should only need to be visible to the package containing the .bzl file containing the) subrule declaration (prevent complaints that such tools need to have broad visibility). It needn't be visible to the rule implementations that invoke the subrule.  
  Before this is introduced the same semantics should apply to implicit dependencies of rules.  
- **Query sensitivity.** If we’ll be able to separate the loading definition and implementation of the rules in the future,  it should be no harder to do this as well for subrules (reference to apilark).  
  That is, changes in the implementation functions (.bzl) wouldn’t trigger re-loading of all BUILD files using the rule downstream (depending on that .bzl). Changes in the public interface, that is attributes or required providers, would still require re-loading of BUILD files using the rule.  
- **Coding guidelines.** [Coding guidelines for Starlark rules](https://docs.google.com/document/d/1uuX1Gz6Kvivp-oOqGja342VrVO534PLF0m_zFtFIBcc/edit#). The new API encapsulates implicit dependencies, toolchains, making following coding guidelines obsolete:   
- §6 Define implicit dependencies in a dictionary next to the implementation. Define toolchain(s) in a list next to the implementation.  
- §9 ~~The rule should provide a full dict of all its attributes.~~ The rule shall at minimum provide a dict of its implicit dependencies.  
- It’s still on the user to:  
- §1 Expose each created action as a (public) function.  
- **Testing.** it should be possible to support with the testing.analysis\_test call; because we can construct a wrapping rule class definition in a macro.

# Alternatives considered

## Doing nothing

Alternative to this proposal was following Coding Guidelines: [Coding guidelines for Starlark rules](https://docs.google.com/document/d/1uuX1Gz6Kvivp-oOqGja342VrVO534PLF0m_zFtFIBcc/edit). The comments in the guidelines revealed that they are too complex to follow and that there is a need for a new “building block”.

This proposal makes several of the proposed Coding Guidelines obsolete (see the sections in this doc for further explanation).

## ctx.attrs vs parameters {#ctx.attrs-vs-parameters}

Using parameters instead of ctx creates a symmetry with public/non-restricted parameters which can't be made available on the ctx.

I believe changing this will actually improve readability (because parameters and their definitions are closer together, not redirected via ctx). 

It is a change for people familiar with the way rules are written. And it will introduce some friction when refactoring. Hopefully, this friction is minimal.

It's also a preparation for further enforcing coding guidelines, where it's desirable to use parameters instead of ctx, because this way it's easier to modify them. So in the future the symmetry can be restored by changing also the rule call.

That will make ctx object "uninviting" to pass around, which is a big problem for readability.

## ctx.subrules.foo {#ctx.subrules.foo}

Introducing a new namespace on the ctx object for everything that is called from the implementation. We’d still have subrules \= \[foo\], but in the implementation functions, we’d need to do ctx.subrules.foo.

This alternative is complex to implement (big extension to the RuleContext object) and it proliferates passing the ctx around, which we consider a bad practice.

## No subrules parameter {#no-subrules-parameter}

Without subrules parameter, Bazel needs to automatically discover what analysis\_func objects are used by a rule. Following references from the rule’s object in Java’s heap, it should be possible to determine all such objects. (In practice, we wouldn’t examine the heap directly). However, this could be anblaze overestimation, leading to unnecessary implicit dependencies.

Further investigation showed that problematic are product types (struct and providers). Commonly used pattern in Starlark is wrapping a module into a struct. Without eagerly evaluating projections, we’d certainly overestimate.

On the flipside, subrules parameter is not perfect. The detection if the subrule is correctly defined happens at runtime. This might cause that an undeclared call to subrule slips through an unevaluated code path. 

# Final thoughts

- subrule becomes a rule, when we define also the public attributes/parameters  
- We should bring rules closer to subrules \- that is simplifying RuleContext by removing: attr, file, files, executable, passing public attributes by parameter and moving documentation into DocStrings.  
- That would make even more coding guidelines obsolete and bring us closer to the ultimate goal of composing Starlark rules.  
- There are several problems related to public attributes, that prevent referring to a rule from subrules parameter:  
  - incoming configuration transitions (they break encapsulation)  
  - split transitions (the problem is that changing an attribute from  a regular to a split configuration is a conservative extension from rule author’s perspective, because it wouldn’t break targets, but it would break anybody using the rule from another rule)  
  - runfiles/data handling (we skipped this in subrules)  
  - macro wrappers (rule expects that the macro is applied before it’s called, but we don’t know how to do this when the rule is composed)  
  - computed default attributes (because multiple public attributes are involved, which we need to compute before analysis. The latter is difficult when a call to another rule happens during analysis)  
  - output attributes (we skipped them from subrules, but they don’t seem that problematic to add for public attributes)

# Document History

| Date | Description |
| :---- | :---- |
| 2023-01-18 | First proposal |
| 2023-02-27 | Renamed to subrule, added details on SubruleCtx |
| 2023-05-20 | Updated the doc with comments |
| 2023-05-31 | Added printing out of blaze query \--output=xml \--xml:default\_values |

