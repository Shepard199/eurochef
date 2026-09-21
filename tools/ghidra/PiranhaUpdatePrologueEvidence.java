import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class PiranhaUpdatePrologueEvidence extends GhidraScript {
  @Override public void run()throws Exception{
    disassemble(toAddr(0x004676F0L)); Instruction cur=currentProgram.getListing().getInstructionAt(toAddr(0x004676F0L));
    for(int i=0;i<80 && cur!=null;i++,cur=cur.getNext())println(cur.getAddress()+"  "+cur);
  }
}