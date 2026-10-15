import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class InputGetterAsmEvidence extends GhidraScript {
 public void run()throws Exception{
  Listing l=currentProgram.getListing();
  InstructionIterator it=l.getInstructions(toAddr(0x004e9090L),true);
  while(it.hasNext()){
   Instruction ins=it.next();
   if(ins.getAddress().compareTo(toAddr(0x004e9120L))>=0)break;
   println(ins.getAddress()+"  "+ins);
  }
 }
}