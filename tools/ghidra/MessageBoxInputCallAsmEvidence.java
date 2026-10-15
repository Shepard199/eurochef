import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class MessageBoxInputCallAsmEvidence extends GhidraScript {
 public void run() throws Exception {
  Listing l=currentProgram.getListing();
  InstructionIterator it=l.getInstructions(toAddr(0x004a6e80L),true);
  while(it.hasNext()){
   Instruction ins=it.next();
   if(ins.getAddress().compareTo(toAddr(0x004a7120L))>=0)break;
   println(ins.getAddress()+"  "+ins);
  }
 }
}