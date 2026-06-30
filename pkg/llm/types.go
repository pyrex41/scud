package llm

// ChatMessage is an OpenAI/xAI compatible message.
type ChatMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// ChatRequest is an OpenAI/xAI compatible completion request.
type ChatRequest struct {
	Model          string          `json:"model"`
	Messages       []ChatMessage   `json:"messages"`
	MaxTokens      int             `json:"max_tokens,omitempty"`
	Temperature    float64         `json:"temperature,omitempty"`
	ResponseFormat *ResponseFormat `json:"response_format,omitempty"`
}

// ResponseFormat constrains a chat-completions response to JSON.
// Type "json_object" guarantees syntactically valid JSON (supported by
// both xAI and OpenAI's chat/completions endpoint).
type ResponseFormat struct {
	Type string `json:"type"`
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

// --- xAI Responses API types ---

// ResponsesRequest is a request to the xAI Responses API.
type ResponsesRequest struct {
	Model     string              `json:"model"`
	Input     []ResponsesMessage  `json:"input"`
	Tools     []ResponsesTool     `json:"tools,omitempty"`
	Reasoning *ResponsesReasoning `json:"reasoning,omitempty"`
}

// ResponsesMessage is a message in a Responses API request.
type ResponsesMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// ResponsesTool is a server-side tool for the Responses API.
type ResponsesTool struct {
	Type string `json:"type"` // "web_search", "x_search", "code_execution", "collections_search"
}

// ResponsesReasoning controls agent count via effort level.
type ResponsesReasoning struct {
	Effort string `json:"effort"` // "low"/"medium" = 4 agents, "high"/"xhigh" = 16 agents
}

// ResponsesResponse is a response from the xAI Responses API.
type ResponsesResponse struct {
	ID     string            `json:"id"`
	Model  string            `json:"model"`
	Status string            `json:"status"`
	Output []ResponsesOutput `json:"output"`
	Usage  ResponsesUsage    `json:"usage"`
	Error  *ResponsesError   `json:"error,omitempty"`
}

// ResponsesOutput is an output block in a Responses API response.
type ResponsesOutput struct {
	ID      string             `json:"id"`
	Role    string             `json:"role"`
	Type    string             `json:"type"`
	Status  string             `json:"status"`
	Content []ResponsesContent `json:"content"`
}

// ResponsesContent is a content block within an output.
type ResponsesContent struct {
	Type string `json:"type"` // "output_text"
	Text string `json:"text"`
}

// ResponsesUsage tracks token usage for a Responses API call.
type ResponsesUsage struct {
	InputTokens  int `json:"input_tokens"`
	OutputTokens int `json:"output_tokens"`
	TotalTokens  int `json:"total_tokens"`
}

// ResponsesError is an error in a Responses API response.
type ResponsesError struct {
	Code    interface{} `json:"code"` // can be int or string
	Message string      `json:"message"`
}
