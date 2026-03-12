package llm

// ChatMessage is an OpenAI/xAI compatible message.
type ChatMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// ChatRequest is an OpenAI/xAI compatible completion request.
type ChatRequest struct {
	Model       string        `json:"model"`
	Messages    []ChatMessage `json:"messages"`
	MaxTokens   int           `json:"max_tokens,omitempty"`
	Temperature float64       `json:"temperature,omitempty"`
}

// ChatResponse is an OpenAI/xAI compatible completion response.
type ChatResponse struct {
	Choices []ChatChoice `json:"choices"`
}

// ChatChoice is a single choice in a ChatResponse.
type ChatChoice struct {
	Message ChatMessage `json:"message"`
}

// AnthropicRequest is a request to the Anthropic messages API.
type AnthropicRequest struct {
	Model     string             `json:"model"`
	Messages  []AnthropicMessage `json:"messages"`
	MaxTokens int                `json:"max_tokens"`
	System    string             `json:"system,omitempty"`
}

// AnthropicMessage is a message in an Anthropic request/response.
type AnthropicMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// AnthropicResponse is a response from the Anthropic messages API.
type AnthropicResponse struct {
	Content []AnthropicContent `json:"content"`
}

// AnthropicContent is a content block in an Anthropic response.
type AnthropicContent struct {
	Type string `json:"type"`
	Text string `json:"text"`
}
