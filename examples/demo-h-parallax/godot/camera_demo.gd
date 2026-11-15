extends Camera2D
## Câmera de demonstração: anda sozinha para a direita para mostrar o parallax.
## As setas esquerda/direita mudam a direção e aceleram.

@export var velocidade := 30.0


func _process(delta: float) -> void:
	var direcao := Input.get_axis("ui_left", "ui_right")
	if direcao == 0.0:
		position.x += velocidade * delta
	else:
		position.x += direcao * velocidade * 4.0 * delta
