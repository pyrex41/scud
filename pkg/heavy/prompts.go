package heavy

import "fmt"

const maxOutputLen = 4000

// RoutingPrompt builds the prompt for Captain to select specialist agents.
func RoutingPrompt(query string, specialists []Agent) string {
	prompt := "You are the Captain coordinator for a multi-agent reasoning ensemble. " +
		"Analyze the following query and determine which specialist agents should be activated.\n\n" +
		"The core four agents (Captain, Harper, Benjamin, Lucas) are ALWAYS active.\n" +
		"Select additional domain specialists ONLY when their expertise is clearly relevant.\n\n" +
		"Available domain specialists:\n"

	for _, s := range specialists {
		prompt += fmt.Sprintf("- %s (%s)\n", s.Name, s.Domain)
	}

	prompt += fmt.Sprintf(
		"\nQuery: %s\n\n"+
			"Return ONLY a JSON array of agent names to activate (including the core four). Example:\n"+
			`["Captain", "Harper", "Benjamin", "Lucas", "Curie", "Lovelace"]`+"\n\n"+
			"Do not include any other text, just the JSON array.",
		query,
	)
	return prompt
}

// SynthesisPrompt builds the prompt for Captain to synthesize agent outputs.
func SynthesisPrompt(query string, outputs []AgentOutput) string {
	prompt := fmt.Sprintf(
		"You are the Captain synthesizer. Multiple specialist agents have analyzed the following query. "+
			"Synthesize their insights into a unified, comprehensive answer.\n\n"+
			"Original query: %s\n\n"+
			"Agent outputs:\n\n",
		query,
	)

	for _, o := range outputs {
		if o.Failed {
			continue
		}
		text := o.Output
		if len(text) > maxOutputLen {
			text = text[:maxOutputLen] + "\n... (truncated)"
		}
		prompt += fmt.Sprintf("## %s (%s)\n%s\n\n", o.Name, o.Domain, text)
	}

	prompt += "Synthesize these perspectives into a single, well-structured answer. " +
		"Attribute key insights to their source agents where appropriate. " +
		"Resolve any contradictions through careful reasoning. " +
		"Indicate confidence levels where relevant."
	return prompt
}

// CritiquePrompt builds the prompt for an agent to critique a synthesis.
func CritiquePrompt(query, synthesis string) string {
	return fmt.Sprintf(
		"The following synthesis was produced for the query below. "+
			"Critically evaluate it: identify weaknesses, missing perspectives, errors, "+
			"or areas that need more nuance. Be constructive and specific.\n\n"+
			"Original query: %s\n\n"+
			"Synthesis:\n%s\n\n"+
			"Provide your critique concisely.",
		query, synthesis,
	)
}

// ResynthesisPrompt builds the prompt for Captain to incorporate critiques.
func ResynthesisPrompt(query, synthesis string, critiques []AgentOutput) string {
	prompt := fmt.Sprintf(
		"You previously synthesized an answer for the query below. "+
			"Multiple agents have critiqued your synthesis. Incorporate their feedback "+
			"to produce an improved final answer.\n\n"+
			"Original query: %s\n\n"+
			"Your previous synthesis:\n%s\n\n"+
			"Critiques:\n\n",
		query, synthesis,
	)

	for _, c := range critiques {
		if c.Failed {
			continue
		}
		prompt += fmt.Sprintf("## %s critique\n%s\n\n", c.Name, c.Output)
	}

	prompt += "Produce an improved final answer incorporating the valid critiques."
	return prompt
}
